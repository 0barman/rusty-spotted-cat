use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::{ECode, Error};

pub(crate) async fn calculate_sign(file: &File) -> Result<String, Error> {
    let mut hasher = md5::Context::new();
    let mut buffer = vec![0; 65536];
    let mut file_handle = file.try_clone().await.map_err(|e| {
        Error::from_code(
            ECode::IoError,
            format!("calculate_sign()->file.try_clone() error: {}", e),
        )
    })?;

    loop {
        let n = file_handle.read(&mut buffer).await.map_err(|e| {
            Error::from_code(
                ECode::IoError,
                format!("calculate_sign()->file_handle.read error: {}", e),
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.consume(&buffer[..n]);
    }

    let digest = hasher.compute();
    Ok(format!("{:x}", digest))
}
