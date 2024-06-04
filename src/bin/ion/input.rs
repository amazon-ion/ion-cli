use crate::auto_decompress::{decompress, AutoDecompressingReader};
use anyhow::Result;
use std::io::{BufReader, Read};

// The number of header bytes to inspect with the `infer` crate to detect compression.
const INFER_HEADER_LENGTH: usize = 8;

/// The compression codec detected at the head of the input stream.
pub enum CompressionDetected {
    // Note that `None` may indicate either that compression detection was disabled OR that the
    // input stream did not begin with a compression identifier that the Ion CLI supports.
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
    //       For now, creating a `Reader` requires that we hand over the entire BufReader.
    //       See: https://github.com/amazon-ion/ion-rust/issues/783
    pub fn into_source(self) -> AutoDecompressingReader {
        self.source
    }

    /// Returns either:
    /// * the name of the input file that this `CommandInput` represents
    /// * the string `"-"` if this `CommandInput` represents STDIN.
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
