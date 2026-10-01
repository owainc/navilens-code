//! Encoding, decoding, parsing and validation of a 5x5 `NaviLens`-style codes.

use std::process::ExitCode;

use arbitrary_int::{u2, u24};
use clap::{Parser, Subcommand};

/// Cyan, magenta, yellow, and black RGB colors.
const DEFAULT_PALETTE: [u32; 4] = [0x0000_FFFF, 0x00FF_00FF, 0x00FF_FF00, 0];

/// CRC-16/CDMA2000 with a zero initial register.
const CODE_CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::Algorithm {
    init: 0,
    ..crc::CRC_16_CDMA2000
});

/// Encode or validate a code and render it in the terminal.
#[derive(Debug, Parser)]
#[command(name = "navilens-code", about = "Encode/decode NaviLens-style codes")]
struct Cli {
    /// Operation to perform.
    #[command(subcommand)]
    command: Command,
}

/// Supported command-line operations.
#[derive(Debug, Subcommand)]
enum Command {
    /// Encode exactly six hexadecimal characters.
    Encode {
        /// The 24-bit data payload in big-endian hexadecimal (6 characters).
        #[arg(value_name = "DATA", value_parser = parse_data)]
        data: u24,
    },
    /// Validate six data hex characters followed by four CRC hex characters.
    Decode {
        /// The marker code in big-endian hexadecimal (10 characters), including its checksum.
        #[arg(value_name = "CODE", value_parser = decode)]
        matrix: ModuleMatrix,
    },
    /// Validate 25 alphanumeric symbols, ignoring separators.
    Parse {
        /// The marker matrix in display order.
        #[arg(value_name = "MATRIX", value_parser = parse_matrix)]
        matrix: ModuleMatrix,
    },
}

/// Parse exactly N bytes of ASCII hexadecimal without slicing UTF-8 strings.
fn parse_hex<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err(format!(
            "expected exactly {} hexadecimal characters, got {} bytes",
            N * 2,
            value.len()
        ));
    }
    let mut bytes = [0; N];
    let (pairs, _) = value.as_bytes().as_chunks::<2>();
    for (byte, &[high, low]) in bytes.iter_mut().zip(pairs) {
        let high = hex_digit(high)?;
        let low = hex_digit(low)?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

/// Convert one ASCII hexadecimal digit into its four-bit value.
fn hex_digit(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("expected only hexadecimal characters (0-9, a-f, A-F)".to_owned()),
    }
}

/// Parse a payload whose size guarantees that it fits in 24 bits.
fn parse_data(value: &str) -> Result<u24, String> {
    let [a, b, c] = parse_hex(value)?;
    Ok(u24::new(u32::from_be_bytes([0, a, b, c])))
}

/// Decode and validate a complete hexadecimal code before it can be rendered.
fn decode(value: &str) -> Result<ModuleMatrix, String> {
    let [a, b, c, high, low] = parse_hex(value)?;
    let mut matrix = ModuleMatrix::new();
    matrix.set_data(u24::new(u32::from_be_bytes([0, a, b, c])));
    matrix.set_crc(u16::from_be_bytes([high, low]));
    matrix.validate()?;
    Ok(matrix)
}

/// Collect exactly 25 Unicode alphanumeric symbols using fixed storage.
fn parse_matrix(value: &str) -> Result<ModuleMatrix, String> {
    let mut symbols = value.chars().filter(|symbol| symbol.is_alphanumeric());
    let mut chars = ['\0'; ModuleMatrix::MODULE_COUNT];
    for character in &mut chars {
        *character = symbols
            .next()
            .ok_or("expected exactly 25 alphanumeric characters after stripping separators")?;
    }
    if symbols.next().is_some() {
        return Err(
            "expected exactly 25 alphanumeric characters after stripping separators".to_owned(),
        );
    }
    ModuleMatrix::parse(&chars)
}

/// Execute the selected operation; clap reports invalid input with a failure exit status.
fn main() -> ExitCode {
    let matrix = match Cli::parse().command {
        Command::Encode { data } => ModuleMatrix::encode(data),
        Command::Decode { matrix } | Command::Parse { matrix } => matrix,
    };

    render_matrix(&matrix, &DEFAULT_PALETTE);

    println!(
        "code: \x1b[36m{:06X}\x1b[33m{:04X}\x1b[0m",
        matrix.data(),
        matrix.crc()
    );

    ExitCode::SUCCESS
}

/// A column-major matrix. A module's role is determined solely by its position.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ModuleMatrix {
    /// Each cell is restricted to two bits by its type.
    modules: [u2; Self::MODULE_COUNT],
}

impl ModuleMatrix {
    /// Number of rows and columns.
    const SIZE: usize = 5;
    /// Total number of cells.
    const MODULE_COUNT: usize = Self::SIZE * Self::SIZE;
    /// Palette corners in value order: top-left, top-right, bottom-right, bottom-left.
    const PALETTE_INDICES: [usize; 4] = [0, 20, 24, 4];
    /// Center cell, reserved for format version zero.
    const VERSION_INDEX: usize = 12;
    /// Payload positions grouped into big-endian bytes, most significant pair first.
    const DATA_INDICES: [[usize; 4]; 3] = [[1, 3, 5, 6], [8, 9, 15, 16], [18, 19, 21, 23]];
    /// Checksum positions grouped into big-endian bytes.
    const CRC_INDICES: [[usize; 4]; 2] = [[2, 7, 10, 11], [13, 14, 17, 22]];
    /// Data and palette positions in storage order, preserving the checksum input format.
    const CHECKSUM_INDICES: [[usize; 4]; 4] = [
        [0, 1, 3, 4],
        [5, 6, 8, 9],
        [15, 16, 18, 19],
        [20, 21, 23, 24],
    ];

    /// Construct the version-zero layout with a canonical palette.
    const fn new() -> Self {
        let mut modules = [u2::new(0); Self::MODULE_COUNT];
        modules[Self::PALETTE_INDICES[1]] = u2::new(1);
        modules[Self::PALETTE_INDICES[2]] = u2::new(2);
        modules[Self::PALETTE_INDICES[3]] = u2::new(3);
        Self { modules }
    }

    /// Encode a typed payload with its computed checksum.
    fn encode(data: u24) -> Self {
        let mut matrix = Self::new();
        matrix.set_data(data);
        matrix.set_computed_crc();
        matrix
    }

    /// Transpose between display order and column-major storage order.
    const fn transpose_index(index: usize) -> usize {
        (index % Self::SIZE) * Self::SIZE + index / Self::SIZE
    }

    /// Resolve a fixed-size display through its four distinct corner symbols and validate it.
    fn parse(chars: &[char; Self::MODULE_COUNT]) -> Result<Self, String> {
        let palette = Self::PALETTE_INDICES.map(|index| chars[Self::transpose_index(index)]);
        for (index, symbol) in palette.iter().enumerate() {
            if palette[..index].contains(symbol) {
                return Err(format!(
                    "palette symbols must be unique; '{symbol}' is repeated"
                ));
            }
        }
        let mut matrix = Self::new();
        for (index, symbol) in chars.iter().enumerate() {
            let value = palette
                .iter()
                .zip(0..4)
                .find_map(|(candidate, value)| (candidate == symbol).then_some(value))
                .ok_or_else(|| format!("unknown module symbol '{symbol}' at index {index}"))?;
            matrix.modules[Self::transpose_index(index)] = u2::new(value);
        }
        matrix.validate()?;
        Ok(matrix)
    }

    /// Yield fixed-size display rows from column-major storage.
    fn rows(&self) -> impl Iterator<Item = [u2; Self::SIZE]> + '_ {
        (0..Self::SIZE).map(|row| std::array::from_fn(|col| self.modules[col * Self::SIZE + row]))
    }

    /// Read complete bytes from fixed groups of four module positions.
    fn read_bytes<const N: usize>(&self, indices: &[[usize; 4]; N]) -> [u8; N] {
        indices.map(|group| pack_byte(group.map(|index| self.modules[index])))
    }

    /// Write exactly one complete byte per group of four module positions.
    fn write_bytes<const N: usize>(&mut self, indices: &[[usize; 4]; N], bytes: [u8; N]) {
        for (group, byte) in indices.iter().zip(bytes) {
            for (&index, value) in group.iter().zip(unpack_byte(byte)) {
                self.modules[index] = value;
            }
        }
    }

    /// Read the 24-bit payload in big-endian order.
    fn data(&self) -> u24 {
        let [a, b, c] = self.read_bytes(&Self::DATA_INDICES);
        u24::new(u32::from_be_bytes([0, a, b, c]))
    }

    /// Write the 24-bit payload without narrowing casts.
    fn set_data(&mut self, data: u24) {
        let [_, a, b, c] = data.value().to_be_bytes();
        self.write_bytes(&Self::DATA_INDICES, [a, b, c]);
    }

    /// Read the stored checksum in big-endian order.
    fn crc(&self) -> u16 {
        u16::from_be_bytes(self.read_bytes(&Self::CRC_INDICES))
    }

    /// Write the checksum in big-endian order.
    fn set_crc(&mut self, crc: u16) {
        self.write_bytes(&Self::CRC_INDICES, crc.to_be_bytes());
    }

    /// Compute and store the checksum for the current payload and palette.
    fn set_computed_crc(&mut self) {
        let crc = self.compute_crc();
        self.set_crc(crc);
    }

    /// Reject unsupported versions and corrupted payloads.
    fn validate(&self) -> Result<(), String> {
        let version = self.modules[Self::VERSION_INDEX].value();
        if version != 0 {
            return Err(format!("unsupported version: expected 0, got {version}"));
        }
        let computed = self.compute_crc();
        let stored = self.crc();
        if computed != stored {
            return Err(format!(
                "invalid CRC: received 0x{stored:04X}, computed 0x{computed:04X}"
            ));
        }
        Ok(())
    }

    /// Checksum the four bytes formed by data and palette modules in storage order.
    fn compute_crc(&self) -> u16 {
        CODE_CRC.checksum(&self.read_bytes(&Self::CHECKSUM_INDICES))
    }
}

/// Pack exactly four two-bit values, most significant pair first.
const fn pack_byte(values: [u2; 4]) -> u8 {
    let [a, b, c, d] = values;
    (a.value() << 6) | (b.value() << 4) | (c.value() << 2) | d.value()
}

/// Split one byte into exactly four two-bit values, most significant pair first.
const fn unpack_byte(byte: u8) -> [u2; 4] {
    [
        u2::new(byte >> 6),
        u2::new((byte >> 4) & 3),
        u2::new((byte >> 2) & 3),
        u2::new(byte & 3),
    ]
}

/// Renders the matrix as a `NaviLens` code in the terminal using supplied palette colors.
fn render_matrix(matrix: &ModuleMatrix, palette: &[u32; 4]) {
    const BLACK: &str = "\x1b[48;2;0;0;0m";
    const WHITE: &str = "\x1b[48;2;255;255;255m";

    for _ in 0..(ModuleMatrix::SIZE + 4) {
        print!("{WHITE}  ");
    }
    println!("\x1b[0m");

    print!("{WHITE}  ");
    for _ in 0..(ModuleMatrix::SIZE + 2) {
        print!("{BLACK}  ");
    }
    print!("{WHITE}  ");
    println!("\x1b[0m");

    for row in matrix.rows() {
        print!("{WHITE}  {BLACK}  ");
        for module in row {
            let rgb = palette[usize::from(module.value())];
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;

            print!("\x1b[38;2;{r};{g};{b};48;2;{r};{g};{b}m{} ", module.value());
        }

        print!("{BLACK}  {WHITE}  ");
        println!("\x1b[0m");
    }

    print!("{WHITE}  ");
    for _ in 0..(ModuleMatrix::SIZE + 2) {
        print!("{BLACK}  ");
    }
    print!("{WHITE}  ");
    println!("\x1b[0m");

    for _ in 0..(ModuleMatrix::SIZE + 4) {
        print!("{WHITE}  ");
    }
    println!("\x1b[0m");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_packing_is_big_endian_and_exhaustive() {
        assert_eq!(pack_byte([0, 1, 2, 3].map(u2::new)), 0x1B);
        for byte in u8::MIN..=u8::MAX {
            assert_eq!(pack_byte(unpack_byte(byte)), byte);
        }
    }

    #[test]
    fn layout_covers_every_cell_once_and_preserves_checksum_order() {
        let mut counts = [0; ModuleMatrix::MODULE_COUNT];
        for index in ModuleMatrix::DATA_INDICES
            .into_iter()
            .flatten()
            .chain(ModuleMatrix::CRC_INDICES.into_iter().flatten())
            .chain(ModuleMatrix::PALETTE_INDICES)
            .chain([ModuleMatrix::VERSION_INDEX])
        {
            counts[index] += 1;
        }
        assert_eq!(counts, [1; ModuleMatrix::MODULE_COUNT]);

        let expected: Vec<_> = (0..ModuleMatrix::MODULE_COUNT)
            .filter(|index| {
                ModuleMatrix::DATA_INDICES
                    .iter()
                    .flatten()
                    .any(|data| data == index)
                    || ModuleMatrix::PALETTE_INDICES.contains(index)
            })
            .collect();
        assert_eq!(
            ModuleMatrix::CHECKSUM_INDICES
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn existing_codes_keep_their_checksums() {
        for (hex, checksum) in [("000000", 0xF7CF), ("123456", 0x98BD), ("FFFFFF", 0x5CDC)] {
            let data = parse_data(hex).unwrap();
            let matrix = ModuleMatrix::encode(data);
            assert_eq!(matrix.data(), data);
            assert_eq!(matrix.crc(), checksum);
            assert_eq!(decode(&format!("{hex}{checksum:04X}")).unwrap(), matrix);
        }
        assert_eq!(parse_data("aBcDeF"), parse_data("ABCDEF"));
    }

    #[test]
    fn known_marker_matches_payload_checksum_and_display() {
        // Fixture supplied in README.md's encode, decode, and parse examples.
        // Keep these expected values literal rather than deriving them with the encoder.
        let expected_rows = [
            [0, 2, 3, 0, 1],
            [2, 2, 3, 0, 0],
            [3, 0, 0, 2, 0],
            [2, 2, 3, 0, 0],
            [3, 2, 1, 0, 2],
        ];
        let matrices = [
            ModuleMatrix::encode(parse_data("AAA000").unwrap()),
            decode("AAA000CFD8").unwrap(),
            parse_matrix("0230122300300202230032102").unwrap(),
            parse_matrix("0 2 3 0 1\n2 2 3 0 0\n3 0 0 2 0\n2 2 3 0 0\n3 2 1 0 2").unwrap(),
            parse_matrix("c y b c m\ny y b c c\nb c c y c\ny y b c c\nb y m c y").unwrap(),
        ];
        for matrix in matrices {
            assert_eq!(matrix.data().value(), 0x00AA_A000);
            assert_eq!(matrix.crc(), 0xCFD8);
            assert_eq!(matrix.compute_crc(), 0xCFD8);
            let rows: Vec<_> = matrix.rows().map(|row| row.map(u2::value)).collect();
            assert_eq!(rows, expected_rows);
        }
    }

    #[test]
    fn every_payload_bit_round_trips_without_changing_other_cells() {
        for bit in 0..24 {
            for value in [1 << bit, 0x00FF_FFFF ^ (1 << bit)] {
                let mut matrix = ModuleMatrix::new();
                matrix.set_crc(0xABCD);
                matrix.set_data(u24::new(value));
                assert_eq!(matrix.data().value(), value);
                assert_eq!(matrix.crc(), 0xABCD);
                for (index, palette_value) in ModuleMatrix::PALETTE_INDICES.into_iter().zip(0..4) {
                    assert_eq!(matrix.modules[index].value(), palette_value);
                }
                assert_eq!(matrix.modules[ModuleMatrix::VERSION_INDEX], u2::new(0));
            }
        }
    }

    #[test]
    fn every_checksum_round_trips_without_changing_data() {
        let data = u24::new(0x0012_3456);
        let mut matrix = ModuleMatrix::encode(data);
        for checksum in u16::MIN..=u16::MAX {
            matrix.set_crc(checksum);
            assert_eq!(matrix.crc(), checksum);
            assert_eq!(matrix.data(), data);
        }
    }

    #[test]
    fn malformed_hex_is_rejected_without_panicking() {
        for input in [
            "", "12345", "1234567", "1000000", "-00001", "+00001", "0x1234", "12345g", "é1234",
            "１２",
        ] {
            assert!(parse_data(input).is_err(), "{input:?}");
        }
        for input in [
            "",
            "123456",
            "12345698B",
            "12345698BD0",
            "12345698BG",
            "12345é000",
            "1234560000",
        ] {
            assert!(decode(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn display_order_and_symbol_remapping_are_preserved() {
        let text = "00211\n02001\n21031\n10212\n33312";
        let matrix = parse_matrix(text).unwrap();
        assert_eq!(matrix, decode("12345698BD").unwrap());
        let displayed: String = matrix
            .rows()
            .flatten()
            .map(|value| char::from(b'0' + value.value()))
            .collect();
        assert_eq!(displayed, text.replace('\n', ""));
        let remapped = text
            .replace('0', "α")
            .replace('1', "β")
            .replace('2', "γ")
            .replace('3', "δ");
        assert_eq!(parse_matrix(&remapped).unwrap(), matrix);
    }

    #[test]
    fn invalid_matrix_symbols_and_lengths_are_rejected() {
        let valid = "0021102001210311021233312";
        assert!(parse_matrix(&valid[..24]).is_err());
        assert!(parse_matrix(&format!("{valid}0")).is_err());
        assert!(parse_matrix("").is_err());
        assert!(
            parse_matrix("0000000000000000000000000")
                .unwrap_err()
                .contains("unique")
        );
        assert!(
            parse_matrix(&valid.replacen("00", "0X", 1))
                .unwrap_err()
                .contains("unknown")
        );
    }

    #[test]
    fn every_non_palette_symbol_corruption_is_rejected() {
        let original = ModuleMatrix::encode(u24::new(0x0012_3456));
        for index in 0..ModuleMatrix::MODULE_COUNT {
            if ModuleMatrix::PALETTE_INDICES.contains(&index) {
                continue;
            }
            for value in (0..4).map(u2::new) {
                if value == original.modules[index] {
                    continue;
                }
                let mut corrupted = original.clone();
                corrupted.modules[index] = value;
                assert!(
                    corrupted.validate().is_err(),
                    "index {index}, value {value}"
                );
                let text: String = corrupted
                    .rows()
                    .flatten()
                    .map(|value| char::from(b'0' + value.value()))
                    .collect();
                assert!(parse_matrix(&text).is_err());
            }
        }
    }

    #[test]
    fn cli_validates_inputs_at_the_boundary() {
        for args in [
            vec!["navilens-code", "encode", "123456"],
            vec!["navilens-code", "decode", "12345698BD"],
            vec!["navilens-code", "parse", "0021102001210311021233312"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
        for args in [
            vec!["navilens-code", "encode", "1000000"],
            vec!["navilens-code", "decode", "1234560000"],
            vec!["navilens-code", "parse", "bad"],
        ] {
            let error = Cli::try_parse_from(args).unwrap_err();
            assert_eq!(error.exit_code(), 2);
        }
    }
}
