use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::{InnerErrorCode, MeowError};

pub(crate) async fn calculate_sign(file: &File) -> Result<String, MeowError> {
    crate::meow_flow_log!("sign", "calculate_sign start");
    let mut hasher = md5::Context::new();
    let mut buffer = vec![0; 65536];
    let mut file_handle = file.try_clone().await.map_err(|e| {
        crate::meow_flow_log!("sign", "file.try_clone failed: {}", e);
        MeowError::from_code(
            InnerErrorCode::IoError,
            format!("calculate_sign()->file.try_clone() error: {}", e),
        )
    })?;

    loop {
        let n = file_handle.read(&mut buffer).await.map_err(|e| {
            crate::meow_flow_log!("sign", "file.read failed: {}", e);
            MeowError::from_code(
                InnerErrorCode::IoError,
                format!("calculate_sign()->file_handle.read error: {}", e),
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.consume(&buffer[..n]);
    }

    let digest = hasher.compute();
    crate::meow_flow_log!("sign", "calculate_sign completed");
    Ok(format!("{:x}", digest))
}
