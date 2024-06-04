use crate::auto_decompress::{decompress, AutoDecompressingReader};
use anyhow::Result;
use std::io::{BufReader, Read};

const INFER_HEADER_LENGTH: usize = 8;

pub enum CompressionDetected {
    None,
    Gzip,
    Zstd,
}

pub struct CommandInput {
    source: AutoDecompressingReader,
    name: String,
    #[allow(dead_code)]
    // This field is retained so commands can print debug information about the input source.
    // It is not currently used, which generates a compile warning.
    compression: CompressionDetected,
}

impl CommandInput {
    pub fn new(name: impl Into<String>, source: impl Read + 'static) -> Result<CommandInput> {
        Ok(Self {
            source: BufReader::new(Box::new(source)),
            name: name.into(),
            compression: CompressionDetected::None,
        })
    }

    pub fn decompress(
        name: impl Into<String>,
        source: impl Read + 'static,
    ) -> Result<CommandInput> {
        let (compression, decompressed) = decompress(source, INFER_HEADER_LENGTH)?;
        Ok(Self {
            source: decompressed,
            name: name.into(),
            compression,
        })
    }

    // TODO: Implement IonInput for mutable references to an impl Read
    //       For now, creating a `Reader` requires that we hand over the entire BufReader
    pub fn into_source(self) -> AutoDecompressingReader {
        self.source
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    #[allow(dead_code)]
    // This field is retained so commands can print debug information about the input source.
    // It is not currently used, which generates a compile warning.
    pub fn compression(&self) -> &CompressionDetected {
        &self.compression
    }
}
