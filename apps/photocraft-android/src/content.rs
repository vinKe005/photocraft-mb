//! Android content-provider I/O. JNI handles remain inside the safe activity scope.
use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JString, JValue};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MAX_IMPORT_BYTES: usize = 128 * 1024 * 1024;

fn activity<T>(f: impl for<'a, 'b, 'c, 'd> FnOnce(&'a mut JNIEnv<'b>, &'c JObject<'d>) -> Result<T>) -> std::result::Result<T, String> {
    robius_android_env::with_activity(|env, activity| {
        let result = f(env, activity);
        if result.is_err() {
            // A failed Java call must not poison the next content-provider operation.
            let _ = env.exception_clear();
        }
        result
    })
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

/// Read the photo that launched this activity via Open With or Share.
/// NativeActivity does not provide a Rust activity-result callback for a picker.
pub(super) fn incoming() -> std::result::Result<Option<(String, Vec<u8>)>, String> {
    activity(|env, activity| {
        let intent = env.call_method(activity, "getIntent", "()Landroid/content/Intent;", &[])?.l()?;
        if intent.is_null() {
            return Ok(None);
        }
        let action = env.call_method(&intent, "getAction", "()Ljava/lang/String;", &[])?.l()?;
        if action.is_null() {
            return Ok(None);
        }
        let action: String = env.get_string(&JString::from(action))?.into();
        let uri = match action.as_str() {
            "android.intent.action.VIEW" => env.call_method(&intent, "getData", "()Landroid/net/Uri;", &[])?.l()?,
            "android.intent.action.SEND" => {
                let key = env.new_string("android.intent.extra.STREAM")?;
                env.call_method(&intent, "getParcelableExtra", "(Ljava/lang/String;)Landroid/os/Parcelable;", &[JValue::Object(&key)])?.l()?
            }
            _ => return Ok(None),
        };
        if uri.is_null() {
            return Err("The shared photo has no content URI".into());
        }
        let scheme = env.call_method(&uri, "getScheme", "()Ljava/lang/String;", &[])?.l()?;
        if scheme.is_null() || env.get_string(&JString::from(scheme))?.to_str()? != "content" {
            return Err("Only Android content-provider photos are supported".into());
        }
        let resolver = env.call_method(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let stream = env.call_method(&resolver, "openInputStream", "(Landroid/net/Uri;)Ljava/io/InputStream;", &[JValue::Object(&uri)])?.l()?;
        if stream.is_null() {
            return Err("The photo provider did not return a stream".into());
        }
        let result = read_stream(env, &stream);
        if result.is_err() {
            let _ = env.exception_clear();
        }
        let closed = env.call_method(&stream, "close", "()V", &[]);
        let bytes = result?;
        closed?;
        // The importer sniffs image signatures. Do not interpret content URIs as file paths.
        Ok(Some(("Shared photo".into(), bytes)))
    })
}

struct ContentReader<'a, 'env, 'obj> {
    env: &'a mut JNIEnv<'env>,
    stream: &'a JObject<'obj>,
    buffer: JByteArray<'env>,
}

impl std::io::Read for ContentReader<'_, '_, '_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let capacity = output.len().min(8192);
        let count = self
            .env
            .call_method(
                self.stream,
                "read",
                "([BII)I",
                &[JValue::Object(&self.buffer), JValue::Int(0), JValue::Int(i32::try_from(capacity).map_err(std::io::Error::other)?)],
            )
            .and_then(|value| value.i())
            .map_err(std::io::Error::other)?;
        if count == -1 {
            return Ok(0);
        }
        let count = usize::try_from(count).map_err(std::io::Error::other)?;
        if count == 0 || count > capacity {
            return Err(std::io::Error::other("Invalid content provider read size"));
        }
        let mut chunk = [0_i8; 8192];
        let slice = chunk.get_mut(..count).ok_or_else(|| std::io::Error::other("Invalid read size"))?;
        self.env.get_byte_array_region(&self.buffer, 0, slice).map_err(std::io::Error::other)?;
        let out = output.get_mut(..count).ok_or_else(|| std::io::Error::other("Invalid output size"))?;
        for (out, byte) in out.iter_mut().zip(slice) {
            *out = *byte as u8;
        }
        Ok(count)
    }
}

fn read_stream(env: &mut JNIEnv<'_>, stream: &JObject<'_>) -> Result<Vec<u8>> {
    let buffer = env.new_byte_array(8192)?;
    let mut reader = ContentReader { env, stream, buffer };
    Ok(crate::stream::read_limited(&mut reader, MAX_IMPORT_BYTES)?)
}

/// Export into Android's Downloads collection, without broad storage permissions.
pub(super) fn write(name: &str, bytes: &[u8]) -> std::result::Result<(), String> {
    activity(|env, activity| {
        let resolver = env.call_method(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let values = env.new_object("android/content/ContentValues", "()V", &[])?;
        let name = std::path::Path::new(name).file_name().and_then(|s| s.to_str()).ok_or("Invalid export name")?;
        for (key, value) in [("_display_name", name), ("mime_type", mime_type(name))] {
            let key = env.new_string(key)?;
            let value = env.new_string(value)?;
            env.call_method(&values, "put", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&key), JValue::Object(&value)])?;
        }
        let pending_key = env.new_string("is_pending")?;
        let pending = env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(1)])?;
        env.call_method(&values, "put", "(Ljava/lang/String;Ljava/lang/Integer;)V", &[JValue::Object(&pending_key), JValue::Object(&pending)])?;
        let collection = env.get_static_field("android/provider/MediaStore$Downloads", "EXTERNAL_CONTENT_URI", "Landroid/net/Uri;")?.l()?;
        let uri = env
            .call_method(
                &resolver,
                "insert",
                "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
                &[JValue::Object(&collection), JValue::Object(&values)],
            )?
            .l()?;
        if uri.is_null() {
            return Err("Android could not create the export".into());
        }
        let result = write_uri(env, &resolver, &uri, bytes);
        if result.is_err() {
            let _ = env.exception_clear();
            let _ = env.call_method(
                &resolver,
                "delete",
                "(Landroid/net/Uri;Ljava/lang/String;[Ljava/lang/String;)I",
                &[JValue::Object(&uri), JValue::Object(&JObject::null()), JValue::Object(&JObject::null())],
            );
            return result;
        }
        env.call_method(&values, "clear", "()V", &[])?;
        let complete = env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(0)])?;
        env.call_method(&values, "put", "(Ljava/lang/String;Ljava/lang/Integer;)V", &[JValue::Object(&pending_key), JValue::Object(&complete)])?;
        let rows = env
            .call_method(
                &resolver,
                "update",
                "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
                &[JValue::Object(&uri), JValue::Object(&values), JValue::Object(&JObject::null()), JValue::Object(&JObject::null())],
            )?
            .i()?;
        if rows != 1 {
            return Err("Android could not finish publishing the export".into());
        }
        Ok(())
    })
}

fn write_uri(env: &mut JNIEnv<'_>, resolver: &JObject<'_>, uri: &JObject<'_>, bytes: &[u8]) -> Result<()> {
    let stream = env.call_method(resolver, "openOutputStream", "(Landroid/net/Uri;)Ljava/io/OutputStream;", &[JValue::Object(uri)])?.l()?;
    if stream.is_null() {
        return Err("Android could not open the export".into());
    }
    let result = (|| -> Result<()> {
        for chunk in bytes.chunks(64 * 1024) {
            // Release each local array so large exports do not exhaust JNI's local refs.
            let array = env.auto_local(env.byte_array_from_slice(chunk)?);
            env.call_method(&stream, "write", "([B)V", &[JValue::Object(array.as_ref())])?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = env.exception_clear();
    }
    let closed = env.call_method(&stream, "close", "()V", &[]);
    result?;
    closed?;
    Ok(())
}

fn mime_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

pub(super) struct Photo {
    pub name: String,
    pub uri: String,
}

fn sdk(env: &mut JNIEnv<'_>) -> Result<i32> {
    Ok(env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?.i()?)
}

fn permitted(env: &mut JNIEnv<'_>, activity: &JObject<'_>, permission: &str) -> Result<bool> {
    let permission = env.new_string(permission)?;
    Ok(env.call_method(activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::Object(&permission)])?.i()? == 0)
}

pub(super) fn request_photos() -> std::result::Result<(), String> {
    activity(|env, activity| {
        let version = sdk(env)?;
        let permissions: &[&str] = if version >= 34 {
            &["android.permission.READ_MEDIA_IMAGES", "android.permission.READ_MEDIA_VISUAL_USER_SELECTED"]
        } else if version >= 33 {
            &["android.permission.READ_MEDIA_IMAGES"]
        } else {
            &["android.permission.READ_EXTERNAL_STORAGE"]
        };
        let array = env.new_object_array(i32::try_from(permissions.len())?, "java/lang/String", JObject::null())?;
        for (index, permission) in permissions.iter().enumerate() {
            let text = env.new_string(permission)?;
            env.set_object_array_element(&array, i32::try_from(index)?, text)?;
        }
        env.call_method(activity, "requestPermissions", "([Ljava/lang/String;I)V", &[JValue::Object(&array), JValue::Int(1)])?;
        Ok(())
    })
}

/// Return only images Android currently permits, including Android 14 selected-photo access.
pub(super) fn photos() -> std::result::Result<Option<Vec<Photo>>, String> {
    activity(|env, activity| {
        let version = sdk(env)?;
        let permission = if version >= 33 { "android.permission.READ_MEDIA_IMAGES" } else { "android.permission.READ_EXTERNAL_STORAGE" };
        if !(permitted(env, activity, permission)? || (version >= 34 && permitted(env, activity, "android.permission.READ_MEDIA_VISUAL_USER_SELECTED")?)) {
            return Ok(None);
        }
        let resolver = env.call_method(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let collection = env.get_static_field("android/provider/MediaStore$Images$Media", "EXTERNAL_CONTENT_URI", "Landroid/net/Uri;")?.l()?;
        let projection = env.new_object_array(2, "java/lang/String", JObject::null())?;
        for (index, column) in ["_id", "_display_name"].iter().enumerate() {
            let name = env.new_string(column)?;
            env.set_object_array_element(&projection, i32::try_from(index)?, name)?;
        }
        let sort = env.new_string("date_added DESC")?;
        let cursor = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&collection),
                    JValue::Object(&projection),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&sort),
                ],
            )?
            .l()?;
        if cursor.is_null() {
            return Err("Android could not list photos".into());
        }
        let result = (|| -> Result<Vec<Photo>> {
            let mut photos = Vec::new();
            // Bound provider work and JNI local references. Refresh re-queries current grants.
            while photos.len() < 256 && env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
                let id = env.call_method(&cursor, "getLong", "(I)J", &[JValue::Int(0)])?.j()?;
                let name = env.call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(1)])?.l()?;
                let name = env.auto_local(name);
                let name: String = if name.as_ref().is_null() { "Photo".into() } else { env.get_string(<&JString>::from(name.as_ref()))?.into() };
                photos.push(Photo { name, uri: format!("content://media/external/images/media/{id}") });
            }
            Ok(photos)
        })();
        if result.is_err() {
            let _ = env.exception_clear();
        }
        let closed = env.call_method(&cursor, "close", "()V", &[]);
        let photos = result?;
        closed?;
        Ok(Some(photos))
    })
}

pub(super) fn read_photo(uri: &str) -> std::result::Result<Vec<u8>, String> {
    activity(|env, activity| {
        let text = env.new_string(uri)?;
        let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&text)])?.l()?;
        let resolver = env.call_method(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let stream = env.call_method(&resolver, "openInputStream", "(Landroid/net/Uri;)Ljava/io/InputStream;", &[JValue::Object(&uri)])?.l()?;
        if stream.is_null() {
            return Err("The selected photo is no longer accessible".into());
        }
        let result = read_stream(env, &stream);
        if result.is_err() {
            let _ = env.exception_clear();
        }
        let closed = env.call_method(&stream, "close", "()V", &[]);
        let bytes = result?;
        closed?;
        Ok(bytes)
    })
}
