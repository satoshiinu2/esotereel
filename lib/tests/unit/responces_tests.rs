use super::*;
use crate::util::result::EsotereelError;

#[test]
fn failed_response_round_trips_error_variants() {
    let asset_id = uuid::Uuid::new_v4();
    let responses = [
        Response::Failed {
            error: EsotereelError::UploadFailed("upload rejected".to_string()),
        },
        Response::Failed {
            error: EsotereelError::AssetNotFound(asset_id),
        },
    ];

    for response in responses {
        let bytes = rkyv::to_bytes::<_, 1024>(&response).unwrap();
        let archived = rkyv::check_archived_root::<Response>(&bytes).unwrap();
        let decoded: Response = archived.deserialize(&mut rkyv::Infallible).unwrap();

        match decoded {
            Response::Failed {
                error: EsotereelError::UploadFailed(message),
            } => assert_eq!(message, "upload rejected"),
            Response::Failed {
                error: EsotereelError::AssetNotFound(id),
            } => assert_eq!(id, asset_id),
            _ => panic!("unexpected response variant"),
        }
    }
}
