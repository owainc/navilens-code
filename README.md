# navilens-code

Independent encoder, decoder and parser for the [NaviLens](https://www.navilens.com/) / [ddTags](https://www.ddtags.com/)
marker format.

This is an independent implementation intended for research, interoperability and experimentation. It is not affiliated
with, endorsed by, sponsored by, or produced by NaviLens Tech SL or the ddTags project.

The tool operates entirely on the marker data itself. It does not provide access to NaviLens services, marker databases
or associated content.

This work originates from research carried out during 2024 while developing custom fiducial marker systems. For enquiries
about custom marker or machine-vision systems, contact: `<my.name>@gmail.com`.

## Contents

- [Commands](#commands)
  - [Encode](#encode)
  - [Decode](#decode)
  - [Parse](#parse)
- [Marker layout](#marker-layout)
- [Scope](#scope)
- [Status](#status)
- [Legal and trademarks](#legal-and-trademarks)
- [Licence](#licence)


## Commands

The CLI provides three operations:

- `encode` - encode a 6-digit hexadecimal data value and generate its CRC
- `decode` - decode a 10-digit hexadecimal value containing data + CRC
- `parse` - parse a complete 5x5 code matrix

All commands render the resulting module matrix to the console and display the complete code.

### `encode`

```text
encode <data>
```

Encodes a 6-digit hexadecimal data value.

The CRC is calculated automatically and the complete code matrix is rendered to the console.

Example:

```text
encode AAA000
```

Input:

```text
AAA000
```

The six hexadecimal digits represent 24 bits of marker data.

The command outputs:

- complete encoded value (data followed by the generated 16-bit CRC)
- rendered 5x5 module matrix

### `decode`

```text
decode <code>
```

Decodes a 10-digit hexadecimal code consisting of:

```text
 AAA000  CFD8
└─data─┘ └crc┘
 24 bits 16 bits
```

Example:

```text
decode AAA000CFD8
```

The command extracts the data and supplied CRC, calculates the expected CRC and renders the resulting code matrix.

### `parse`

```text
parse <matrix>
```

Parses a complete 5x5 code matrix in **row-major / row-centric order**.

The input must contain 25 module symbols after normalisation.

Non-alphanumeric characters are ignored, making formatted matrices valid input. For example, these are equivalent:

```text
0230122300300202230032102
```

and:

```text
0 2 3 0 1
2 2 3 0 0
3 0 0 2 0
2 2 3 0 0
3 2 1 0 2
```

and:

```text
c y b c m
y y b c c
b c c y c
y y b c c
b y m c y
```

The symbols themselves are arbitrary; `parse` determines the corresponding module values from their positions and
relationships within the marker.

After parsing, the command outputs:

- complete encoded value (decoded data followed by the validated CRC)
- rendered module matrix

# Marker layout

The marker is a 5x5 matrix containing:

- 12 data modules (`0`-`b`)
- 8 CRC modules (`c0`-`c7`)
- 4 palette modules (`p0`-`p3`)
- 1 version module (`v`)

The physical matrix is supplied and displayed **row-major**, but the data
modules are encoded in **column-major / column-centric order**.

|   Row | Column 0 | Column 1 | Column 2 | Column 3 | Column 4 |
|------:|:--------:|:--------:|:--------:|:--------:|:--------:|
| **0** |   `p0`   |   `2`    |   `c2`   |   `6`    |   `p1`   |
| **1** |   `0`    |   `3`    |   `c3`   |   `7`    |   `a`    |
| **2** |   `c0`   |   `c1`   |   `v`    |   `c6`   |   `c7`   |
| **3** |   `1`    |   `4`    |   `c4`   |   `8`    |   `b`    |
| **4** |   `p3`   |   `5`    |   `c5`   |   `9`    |   `p2`   |

The data-module sequence is therefore:

```text
1, 3, 5, 6, 8, 9, 15, 16, 18, 19, 21, 23
```

## Module types

### Data

`0` through `b` are the twelve data modules.

Each module represents two bits, giving:

```text
12 x 2 = 24 bits
```

or six hexadecimal digits of input data.

### CRC

`c0` through `c7` contain the 16-bit check value:

```text
8 x 2 = 16 bits
```

or four hexadecimal digits. The implementation uses the CRC scheme observed for this code format (CRC-16-CDMA2000 with
zero initial register).

### Palette

`p0` through `p3` are the four palette/reference modules.

These establish the four module values used by the code, nominally these are cyan, magenta, yellow and black.

### Version

The centre module, `v`, appears to represent a format/version value. All samples observed during this research used `v = 0`.
No other version value has been observed, so the behaviour or meaning of other values is currently unknown.

# Scope

This project implements the visual marker encoding used by NaviLens/ddTags.

It does **not** implement or provide:

- NaviLens online services
- NaviLens code registration
- NaviLens location or accessibility content
- NaviLens databases
- access to information associated with registered NaviLens marker IDs
- the official NaviLens or NaviLens GO applications

Generating a syntactically valid marker does not imply that the marker has been registered with, recognised by, or
assigned content within any NaviLens service.

# Status

This implementation is based on independent research into observable marker behaviour and format characteristics.

The format described here should therefore be treated as an independently derived description of observed behaviour
rather than official NaviLens or ddTags documentation.

In particular:

- only version `0` has been observed;
- behaviour of other marker sizes or versions is not asserted;
- compatibility with current or future NaviLens software is not guaranteed.

# Legal and trademarks

This project is independent and unofficial.

**NaviLens** and associated names, logos and other distinctive marks are the property of their respective owners. No
licence or right to use any third-party trademark is granted by this project.

NaviLens Tech SL describes NaviLens and its intellectual and industrial property rights at:

- <https://www.navilens.com/>
- <https://www.navilens.com/en/legal/terms-of-use>
- <https://www.navilens.com/en/legal/legal-notice>

Information about ddTags is available at:

- <https://www.ddtags.com/>

References to NaviLens and ddTags in this repository are descriptive: they identify the marker format with which this
independent implementation is intended to interoperate.

Neither the source-code licence nor this repository grants rights to any third-party:

- trademarks or branding;
- patents or patent applications;
- copyrighted application code or assets;
- services, databases or hosted content; or
- other intellectual or industrial property.

No NaviLens or ddTags source code is included in this project.

Use of this software does not grant permission to represent generated markers as official NaviLens/ddTags markers or to
imply endorsement, registration or compatibility with any third-party service.

Users are responsible for determining whether their particular use of this software is permitted under applicable law
and any agreements applicable to services or software they separately use.

# Licence

This project's source code is licensed under the MIT License.

The MIT licence applies only to copyright in this independent implementation. It does not grant rights in third-party
trademarks, patents, services, databases, copyrighted works or other intellectual property.

See [LICENSE](LICENSE).
