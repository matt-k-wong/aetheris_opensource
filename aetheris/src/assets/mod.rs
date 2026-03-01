pub mod dehacked;
pub mod wad;

use async_trait::async_trait;

#[async_trait]
pub trait AssetWarehouse {
    /// Fetches the raw bytes of an asset by its path or identifier.
    async fn load_raw(&self, identifier: &str) -> anyhow::Result<Vec<u8>>;
}

/// A desktop implementation that loads assets from the local file system.
pub struct FileSystemWarehouse;

#[async_trait]

impl AssetWarehouse for FileSystemWarehouse {
    async fn load_raw(&self, identifier: &str) -> anyhow::Result<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Read;

            let mut file = std::fs::File::open(identifier)?;

            let mut buffer = Vec::new();

            file.read_to_end(&mut buffer)?;

            Ok(buffer)
        }

        #[cfg(target_arch = "wasm32")]
        {
            anyhow::bail!("FileSystemWarehouse not supported on WASM")
        }
    }
}

/// A web implementation that loads assets via fetch().

pub struct WebWarehouse;

#[async_trait]

impl AssetWarehouse for WebWarehouse {
    async fn load_raw(&self, _identifier: &str) -> anyhow::Result<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;

            use wasm_bindgen_futures::JsFuture;

            use web_sys::{Request, RequestInit, RequestMode, Response};

            let mut opts = RequestInit::new();

            opts.method("GET");

            opts.mode(RequestMode::Cors);

            let request = Request::new_with_str_and_init(_identifier, &opts)
                .map_err(|e| anyhow::anyhow!("Request creation failed: {:?}", e))?;

            let window = web_sys::window().unwrap();

            let resp_value = JsFuture::from(window.fetch_with_request(&request))
                .await
                .map_err(|e| anyhow::anyhow!("Fetch failed: {:?}", e))?;

            let resp: Response = resp_value
                .dyn_into()
                .map_err(|e| anyhow::anyhow!("Response cast failed: {:?}", e))?;

            if !resp.ok() {
                anyhow::bail!("Fetch returned status {}", resp.status());
            }

            let array_buffer_promise = resp
                .array_buffer()
                .map_err(|e| anyhow::anyhow!("array_buffer() failed: {:?}", e))?;

            let array_buffer_value = JsFuture::from(array_buffer_promise)
                .await
                .map_err(|e| anyhow::anyhow!("ArrayBuffer promise failed: {:?}", e))?;

            let uint8_array = js_sys::Uint8Array::new(&array_buffer_value);

            Ok(uint8_array.to_vec())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            anyhow::bail!("WebWarehouse only supported on WASM")
        }
    }
}
