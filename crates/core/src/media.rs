use std::path::Path;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::media::{self, AudioOptions, DocumentOptions, ImageOptions, VideoOptions};
use whatsapp_rust::prelude::wa;
use whatsapp_rust::upload::UploadResponse;

/// How a file will be sent, decided from its extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaKind {
    pub media_type: MediaType,
    pub mimetype: String,
}

pub fn classify(path: &Path) -> MediaKind {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let media_type = match mime.type_().as_str() {
        "image" if mime.subtype() != "svg" => MediaType::Image,
        "video" => MediaType::Video,
        "audio" => MediaType::Audio,
        _ => MediaType::Document,
    };
    MediaKind {
        media_type,
        mimetype: mime.essence_str().to_string(),
    }
}

pub fn build_message(
    kind: &MediaKind,
    upload: UploadResponse,
    path: &Path,
    caption: Option<String>,
) -> wa::Message {
    let mimetype = Some(kind.mimetype.clone());
    match kind.media_type {
        MediaType::Image => media::image_message(
            upload,
            ImageOptions {
                caption,
                mimetype,
                ..Default::default()
            },
        ),
        MediaType::Video => media::video_message(
            upload,
            VideoOptions {
                caption,
                mimetype,
                ..Default::default()
            },
        ),
        // Audio messages carry no caption; v1 dropped it for audio too.
        MediaType::Audio => media::audio_message(
            upload,
            AudioOptions {
                mimetype,
                ..Default::default()
            },
        ),
        _ => {
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            media::document_message(
                upload,
                DocumentOptions {
                    mimetype,
                    title: file_name.clone(),
                    file_name,
                    caption,
                    ..Default::default()
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension() {
        assert_eq!(classify(Path::new("a.JPG")).media_type, MediaType::Image);
        assert_eq!(
            classify(Path::new(r"C:\x\clip.mp4")).media_type,
            MediaType::Video
        );
        assert_eq!(classify(Path::new("v.ogg")).media_type, MediaType::Audio);
        assert_eq!(
            classify(Path::new("invoice.pdf")).media_type,
            MediaType::Document
        );
        assert_eq!(
            classify(Path::new("logo.svg")).media_type,
            MediaType::Document
        );
        assert_eq!(
            classify(Path::new("noext")).mimetype,
            "application/octet-stream"
        );
    }
}
