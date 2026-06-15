use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProtocolInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_baud_rate: u32,
}

pub const SUPPORTED_PROTOCOLS: &[ProtocolInfo] = &[
    ProtocolInfo {
        id: "excell",
        name: "Excell",
        description: "Balanzas Excell (stream continuo ASCII, común en retail peruano)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "generic",
        name: "Genérico continuo",
        description: "Extrae el primer número decimal de cada línea con unidad opcional",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "cas",
        name: "CAS",
        description: "CAS CI/LP/ER (WGT:, WT, stream con kg/g)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "toledo",
        name: "Toledo / Mettler (continuo)",
        description: "Mettler Toledo Prix, PS, Tiger (línea con peso y unidad)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "toledo_stx",
        name: "Toledo STX/ETX",
        description: "Tramas enmarcadas STX…ETX con peso en ASCII",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "mettler_sics",
        name: "Mettler MT-SICS",
        description: "Respuestas SICS (S S, SI) con peso y unidad SI",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "dibal",
        name: "Dibal",
        description: "Dibal G/M/L series (retail, formato peso fijo)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "kretz",
        name: "Kretz",
        description: "Kretz ARS/eKO (muy usada en LATAM, stream numérico)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "magellan",
        name: "Magellan / Datalogic",
        description: "Scanner-balanza retail, líneas con peso embebido",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "avery",
        name: "Avery Berkel",
        description: "Avery Berkel FX/XI (stream continuo con separadores)",
        default_baud_rate: 9600,
    },
    ProtocolInfo {
        id: "rahul",
        name: "Rahul / genérica china",
        description: "Formato +00000.000kg o similar (variante budget)",
        default_baud_rate: 9600,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleProtocol {
    Excell,
    Generic,
    Cas,
    Toledo,
    ToledoStx,
    MettlerSics,
    Dibal,
    Kretz,
    Magellan,
    Avery,
    Rahul,
}

impl ScaleProtocol {
    pub fn parse_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "excell" => Some(Self::Excell),
            "generic" | "generico" | "genérico" => Some(Self::Generic),
            "cas" => Some(Self::Cas),
            "toledo" | "mettler" | "toledo_continuous" => Some(Self::Toledo),
            "toledo_stx" | "toledo-stx" | "stx" => Some(Self::ToledoStx),
            "mettler_sics" | "mt-sics" | "sics" => Some(Self::MettlerSics),
            "dibal" => Some(Self::Dibal),
            "kretz" => Some(Self::Kretz),
            "magellan" | "datalogic" => Some(Self::Magellan),
            "avery" | "avery_berkel" | "berkel" => Some(Self::Avery),
            "rahul" | "china" | "fixed" => Some(Self::Rahul),
            _ => None,
        }
    }

    pub fn info(self) -> &'static ProtocolInfo {
        SUPPORTED_PROTOCOLS
            .iter()
            .find(|p| Self::parse_id(p.id) == Some(self))
            .expect("protocol metadata missing")
    }

    pub fn parse_line(self, raw: &str) -> Option<ParsedWeight> {
        let line = sanitize_line(raw);

        match self {
            Self::Excell => parse_excell(&line).or_else(|| parse_generic(&line)),
            Self::Generic => parse_generic(&line),
            Self::Cas => parse_cas(&line).or_else(|| parse_generic(&line)),
            Self::Toledo => parse_toledo(&line).or_else(|| parse_generic(&line)),
            Self::ToledoStx => parse_toledo_stx(raw).or_else(|| parse_toledo(&line)),
            Self::MettlerSics => parse_mettler_sics(&line).or_else(|| parse_generic(&line)),
            Self::Dibal => parse_dibal(&line).or_else(|| parse_generic(&line)),
            Self::Kretz => parse_kretz(&line).or_else(|| parse_generic(&line)),
            Self::Magellan => parse_magellan(&line).or_else(|| parse_generic(&line)),
            Self::Avery => parse_avery(&line).or_else(|| parse_generic(&line)),
            Self::Rahul => parse_rahul(&line).or_else(|| parse_generic(&line)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWeight {
    pub value: f64,
    pub unit: WeightUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WeightUnit {
    Kg,
    G,
    Lb,
    Unknown,
}

impl WeightUnit {
    pub fn as_kg(self, value: f64) -> f64 {
        match self {
            WeightUnit::Kg | WeightUnit::Unknown => value,
            WeightUnit::G => value / 1000.0,
            WeightUnit::Lb => value * 0.453_592_37,
        }
    }
}

fn sanitize_line(raw: &str) -> String {
    raw.chars()
        .filter(|c| *c != '\x02' && *c != '\x03' && *c != '\x1b')
        .collect::<String>()
        .trim()
        .to_string()
}

fn parse_generic(line: &str) -> Option<ParsedWeight> {
    let lower = line.to_ascii_lowercase();
    let unit = detect_unit(&lower);
    let number = extract_decimal(line)?;
    Some(ParsedWeight {
        value: number,
        unit,
    })
}

/// Excell scales stream a continuous ASCII weight line, often fixed-width.
fn parse_excell(line: &str) -> Option<ParsedWeight> {
    if let Some(parsed) = parse_rahul(line) {
        return Some(parsed);
    }

    // W=0.525, WT:0.525, WEIGHT 0.525
    if let Some(rest) = line.split_once(['=', ':']).map(|(_, rhs)| rhs.trim()) {
        if let Some(parsed) = parse_generic(rest) {
            return Some(parsed);
        }
    }

    parse_generic(line)
}

fn parse_cas(line: &str) -> Option<ParsedWeight> {
    let upper = line.to_ascii_uppercase();
    if let Some(rest) = upper
        .strip_prefix("WGT:")
        .or_else(|| upper.strip_prefix("WT"))
    {
        return parse_generic(rest.trim());
    }
    parse_generic(line)
}

fn parse_toledo(line: &str) -> Option<ParsedWeight> {
    // Lines like "S S     0.525 kg" or "   0.525 kg"
    if let Some(rest) = line.split_whitespace().last() {
        if rest.eq_ignore_ascii_case("kg")
            || rest.eq_ignore_ascii_case("g")
            || rest.eq_ignore_ascii_case("lb")
        {
            let without_unit = line
                .rsplit_once(' ')
                .map(|(lhs, _)| lhs.trim())
                .unwrap_or(line);
            if let Some(parsed) = parse_generic(without_unit) {
                return Some(parsed);
            }
        }
    }
    parse_generic(line)
}

fn parse_toledo_stx(raw: &str) -> Option<ParsedWeight> {
    let start = raw.find('\x02')?;
    let end = raw[start..].find('\x03')?;
    let frame = &raw[start + 1..start + end];
    parse_toledo(frame)
}

fn parse_mettler_sics(line: &str) -> Option<ParsedWeight> {
    // "S S     0.525 kg" or "SI     0.525 kg"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 && (parts[0] == "S" || parts[0] == "SI") {
        return parse_generic(&parts[2..].join(" "));
    }
    parse_generic(line)
}

fn parse_dibal(line: &str) -> Option<ParsedWeight> {
    // Often: <STX>PPPPPP.UUUkg<ETX> or plain decimal with kg
    if let Some(idx) = line.to_ascii_lowercase().find("kg") {
        let prefix = &line[..idx];
        if let Some(number) = extract_decimal(prefix) {
            return Some(ParsedWeight {
                value: number,
                unit: WeightUnit::Kg,
            });
        }
    }
    parse_generic(line)
}

fn parse_kretz(line: &str) -> Option<ParsedWeight> {
    // Kretz streams a numeric field, sometimes prefixed with status chars.
    let digits = line
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '+' && *c != '-')
        .collect::<String>();
    parse_generic(&digits)
}

fn parse_magellan(line: &str) -> Option<ParsedWeight> {
    // Embedded weight fields in longer retail lines.
    if let Some(parsed) = parse_generic(line) {
        return Some(parsed);
    }
    None
}

fn parse_avery(line: &str) -> Option<ParsedWeight> {
    // Avery often uses comma decimal in LATAM configs.
    let normalized = line.replace(',', ".");
    parse_generic(&normalized)
}

fn parse_rahul(line: &str) -> Option<ParsedWeight> {
    // +00000.525kg / -0001.250kg fixed width
    let trimmed = line.trim();
    if trimmed.len() < 5 {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let unit = detect_unit(&lower);
    let number = extract_decimal(trimmed)?;
    Some(ParsedWeight {
        value: number,
        unit,
    })
}

fn detect_unit(lower: &str) -> WeightUnit {
    if lower.contains("kg") {
        WeightUnit::Kg
    } else if lower.contains('g') && !lower.contains("kg") {
        WeightUnit::G
    } else if lower.contains("lb") {
        WeightUnit::Lb
    } else {
        WeightUnit::Unknown
    }
}

fn extract_decimal(input: &str) -> Option<f64> {
    let mut token = String::new();
    let mut started = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() || ((ch == '.' || ch == ',') && started) {
            token.push(if ch == ',' { '.' } else { ch });
            started = true;
        } else if ch == '+' || ch == '-' {
            if !token.is_empty() {
                break;
            }
            token.push(ch);
            started = true;
        } else if started {
            break;
        }
    }

    if token.is_empty() {
        return None;
    }

    token.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excell_stream_line() {
        let parsed = ScaleProtocol::Excell.parse_line("     0.525 kg").unwrap();
        assert!((parsed.value - 0.525).abs() < f64::EPSILON);
        assert_eq!(parsed.unit, WeightUnit::Kg);
    }

    #[test]
    fn cas_wgt_prefix() {
        let parsed = ScaleProtocol::Cas.parse_line("WGT:1.250").unwrap();
        assert!((parsed.value - 1.250).abs() < f64::EPSILON);
    }

    #[test]
    fn rahul_fixed_width() {
        let parsed = ScaleProtocol::Rahul.parse_line("+00001.250kg").unwrap();
        assert!((parsed.value - 1.250).abs() < f64::EPSILON);
    }

    #[test]
    fn mettler_sics_response() {
        let parsed = ScaleProtocol::MettlerSics
            .parse_line("S S     0.125 kg")
            .unwrap();
        assert!((parsed.value - 0.125).abs() < f64::EPSILON);
    }
}
