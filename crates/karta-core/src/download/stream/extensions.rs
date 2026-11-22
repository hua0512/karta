pub(super) fn image_extension_from_url(url: &str) -> &'static str {
    let Some(extension) = url_extension(url) else {
        return "jpg";
    };
    if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
        "jpg"
    } else if extension.eq_ignore_ascii_case("png") {
        "png"
    } else if extension.eq_ignore_ascii_case("gif") {
        "gif"
    } else if extension.eq_ignore_ascii_case("webp") {
        "webp"
    } else {
        "jpg"
    }
}

pub(super) fn audio_extension_from_url(url: &str) -> &'static str {
    let Some(extension) = url_extension(url) else {
        return "mp3";
    };
    if extension.eq_ignore_ascii_case("mp3") {
        "mp3"
    } else if extension.eq_ignore_ascii_case("m4a") {
        "m4a"
    } else if extension.eq_ignore_ascii_case("aac") {
        "aac"
    } else if extension.eq_ignore_ascii_case("flac") {
        "flac"
    } else if extension.eq_ignore_ascii_case("wav") {
        "wav"
    } else {
        "mp3"
    }
}

pub(super) fn video_extension_from_url(url: &str) -> &'static str {
    let Some(extension) = url_extension(url) else {
        return "mp4";
    };
    if extension.eq_ignore_ascii_case("mp4") {
        "mp4"
    } else if extension.eq_ignore_ascii_case("mov") {
        "mov"
    } else if extension.eq_ignore_ascii_case("webm") {
        "webm"
    } else if extension.eq_ignore_ascii_case("m4v") {
        "m4v"
    } else if extension.eq_ignore_ascii_case("mkv") {
        "mkv"
    } else if extension.eq_ignore_ascii_case("flv") {
        "flv"
    } else if extension.eq_ignore_ascii_case("ts") {
        "ts"
    } else {
        "mp4"
    }
}

fn url_extension(url: &str) -> Option<&str> {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    let extension = path.rsplit('.').next()?;
    if extension.is_empty() || extension == path || extension.contains('/') {
        return None;
    }
    Some(extension)
}
