//! Read-only Grapevine wire-format inspection. No rule evaluation or database writes.
use encoding_rs::{Encoding, WINDOWS_1252};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::LazyLock;
pub mod html;

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_OBJECTS: usize = 250_000;
const SCHEMA: &str = include_str!("../wire-schema.json");
static WIRE_SCHEMA: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(SCHEMA).expect("embedded schema"));

#[derive(Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub offset: Option<usize>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub report_version: u32,
    pub format: String,
    pub version: Option<f64>,
    pub status: String,
    pub input_bytes: usize,
    pub sha256: String,
    pub encoding: String,
    pub application_import_ready: bool,
    pub metadata: Map<String, Value>,
    pub sections: BTreeMap<String, Vec<Value>>,
    pub diagnostics: Vec<Diagnostic>,
    pub non_ascii_strings: Vec<Value>,
    pub unparsed: Option<Value>,
    pub raw_xml: Option<String>,
}

impl Report {
    fn new(bytes: &[u8], encoding: &str) -> Self {
        Self {
            report_version: 1,
            format: "unknown".into(),
            version: None,
            status: "invalid".into(),
            input_bytes: bytes.len(),
            sha256: hex(&Sha256::digest(bytes)),
            encoding: encoding.into(),
            application_import_ready: false,
            metadata: Map::new(),
            sections: BTreeMap::new(),
            diagnostics: vec![],
            non_ascii_strings: vec![],
            unparsed: None,
            raw_xml: None,
        }
    }
    fn note(
        &mut self,
        severity: &str,
        code: &str,
        message: impl Into<String>,
        offset: Option<usize>,
        path: &str,
    ) {
        self.diagnostics.push(Diagnostic {
            severity: severity.into(),
            code: code.into(),
            message: message.into(),
            offset,
            path: path.into(),
        });
    }
    pub fn record_count(&self) -> usize {
        self.sections.values().map(Vec::len).sum()
    }
    pub fn succeeded(&self) -> bool {
        self.status == "complete"
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        result.push(DIGITS[(b >> 4) as usize] as char);
        result.push(DIGITS[(b & 15) as usize] as char);
    }
    result
}

#[derive(Debug)]
struct Failure {
    code: &'static str,
    offset: usize,
    path: String,
    message: String,
}
type ReadResult<T> = Result<T, Failure>;

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    version: f64,
    encoding: &'static Encoding,
    objects: usize,
    non_ascii: Vec<Value>,
    diagnostics: Vec<Diagnostic>,
    schema: &'static Value,
}

impl<'a> Reader<'a> {
    fn fail(&self, code: &'static str, path: &str, message: impl Into<String>) -> Failure {
        Failure {
            code,
            offset: self.pos,
            path: path.into(),
            message: message.into(),
        }
    }
    fn take<const N: usize>(&mut self, path: &str) -> ReadResult<[u8; N]> {
        let end = self
            .pos
            .checked_add(N)
            .ok_or_else(|| self.fail("TRUNCATED", path, "Offset overflow"))?;
        let slice = self.bytes.get(self.pos..end).ok_or_else(|| {
            self.fail(
                "TRUNCATED",
                path,
                format!("Need {N} bytes, {} remain", self.bytes.len() - self.pos),
            )
        })?;
        self.pos = end;
        Ok(slice.try_into().unwrap())
    }
    fn i16(&mut self, path: &str) -> ReadResult<i16> {
        Ok(i16::from_le_bytes(self.take(path)?))
    }
    fn i32(&mut self, path: &str) -> ReadResult<i32> {
        Ok(i32::from_le_bytes(self.take(path)?))
    }
    fn f64(&mut self, path: &str) -> ReadResult<f64> {
        let n = f64::from_le_bytes(self.take(path)?);
        if n.is_finite() {
            Ok(n)
        } else {
            Err(self.fail("NONFINITE_NUMBER", path, "Non-finite floating point value"))
        }
    }
    fn count(&mut self, path: &str) -> ReadResult<usize> {
        let n = self.i16(path)?;
        if n < 0 {
            return Err(self.fail("NEGATIVE_COUNT", path, format!("Negative count {n}")));
        }
        let n = n as usize;
        if n > self.bytes.len() - self.pos {
            return Err(self.fail("IMPOSSIBLE_COUNT", path, "Count exceeds remaining bytes"));
        }
        Ok(n)
    }
    fn string(&mut self, path: &str) -> ReadResult<String> {
        let n = self.i16(path)?;
        if n < 0 {
            return Err(self.fail("NEGATIVE_STRING_LENGTH", path, "Negative string length"));
        }
        let start = self.pos;
        let end = start + n as usize;
        let raw = self.bytes.get(start..end).ok_or_else(|| {
            self.fail(
                "TRUNCATED_STRING",
                path,
                format!("String length {n} exceeds remaining bytes"),
            )
        })?;
        let (text, _, errors) = self.encoding.decode(raw);
        if errors {
            return Err(self.fail("TEXT_DECODE_ERROR", path, "Bytes cannot be decoded with the selected encoding; select the original Windows code page"));
        }
        if raw.iter().any(|b| *b >= 128) {
            self.non_ascii
                .push(json!({"offset":start,"length":n,"path":path,"bytes_hex":hex(raw)}));
        }
        self.pos = end;
        Ok(text.into_owned())
    }
    fn value(&mut self, kind: &str, path: &str) -> ReadResult<Value> {
        match kind {
            "str" => Ok(json!(self.string(path)?)),
            "i16" => Ok(json!(self.i16(path)?)),
            "i32" => Ok(json!(self.i32(path)?)),
            "bool" => {
                let raw = self.i16(path)?;
                if raw != 0 && raw != -1 {
                    self.diagnostics.push(Diagnostic {
                        severity: "warning".into(),
                        code: "NONCANONICAL_BOOL".into(),
                        message: format!("VB Boolean is {raw}; original integer retained"),
                        offset: Some(self.pos - 2),
                        path: path.into(),
                    });
                }
                Ok(json!({"value":raw!=0,"vb_raw":raw}))
            }
            "f32" => {
                let n = f32::from_le_bytes(self.take(path)?);
                if !n.is_finite() {
                    return Err(self.fail("NONFINITE_NUMBER", path, "Non-finite Single"));
                }
                Ok(json!(n))
            }
            "f64" => Ok(json!(self.f64(path)?)),
            "date" => Ok(json!({"ole_automation_days":self.f64(path)?})),
            "traits" => self.traits(path),
            "experience" => self.experience(path),
            "boons" => self.array("Boon", path).map(Value::Array),
            name => self.record(name, path),
        }
    }
    fn fields(
        &mut self,
        spec: &str,
        path: &str,
        target: &mut Map<String, Value>,
    ) -> ReadResult<()> {
        for part in spec.split_whitespace() {
            let (name, kind) = part.split_once(':').unwrap();
            target.insert(name.into(), self.value(kind, &format!("{path}.{name}"))?);
        }
        Ok(())
    }
    fn array(&mut self, kind: &str, path: &str) -> ReadResult<Vec<Value>> {
        let count = self.count(path)?;
        let mut items = Vec::new();
        for i in 0..count {
            items.push(self.record(kind, &format!("{path}[{i}]"))?);
        }
        Ok(items)
    }
    fn traits(&mut self, path: &str) -> ReadResult<Value> {
        let mut fields = Map::new();
        self.fields(
            "Name:str Alphabetized:bool Atomic:bool Negative:bool Display:i32",
            path,
            &mut fields,
        )?;
        fields.insert(
            "Traits".into(),
            Value::Array(self.array("Trait", &format!("{path}.Traits"))?),
        );
        Ok(Value::Object(fields))
    }
    fn experience(&mut self, path: &str) -> ReadResult<Value> {
        let mut fields = Map::new();
        self.fields("Unspent:f32 Earned:f32", path, &mut fields)?;
        fields.insert(
            "History".into(),
            Value::Array(self.array("History", &format!("{path}.History"))?),
        );
        Ok(Value::Object(fields))
    }
    fn record(&mut self, kind: &str, path: &str) -> ReadResult<Value> {
        self.objects += 1;
        if self.objects > MAX_OBJECTS {
            return Err(self.fail("OBJECT_LIMIT", path, "Object limit exceeded"));
        }
        let mut f = Map::new();
        match kind {
            "Trait" => self.fields("Name:str Value:str Note:str",path,&mut f)?,
            "History" => self.fields("When:date Change:f32 ChangeType:i32 Reason:str Earned:f32 Unspent:f32",path,&mut f)?,
            "Boon" => self.fields("BoonType:str CharName:str IsOwed:bool BoonDate:date Description:str",path,&mut f)?,
            "Player" => {
                self.fields("Name:str ID:str EMail:str Phone:str Position:str",path,&mut f)?;
                self.fields(if self.version>=2.397 {"Status:str"} else {"Active:bool"},path,&mut f)?;
                self.fields("LastModified:date Experience:experience Address:str Notes:str",path,&mut f)?;
            },
            "Item" => self.fields("Name:str ItemType:str ItemSubtype:str Level:i16 Bonus:i16 DamageType:str DamageAmount:i16 Concealability:str TemperList:traits AbilityList:traits NegativeList:traits Availability:traits Powers:str Appearance:str Notes:str LastModified:date",path,&mut f)?,
            "Rote" => self.fields("Name:str Level:i16 Duration:str SphereList:traits Description:str Grades:str LastModified:date",path,&mut f)?,
            "Location" => {
                self.fields("Name:str LocType:str Level:i16 Owner:str",path,&mut f)?;
                if self.version>=2.399 { self.fields("Access:str",path,&mut f)?; }
                self.fields("Affinity:str Totem:str SecTraits:i16 SecRetests:i16 Gauntlet:i16",path,&mut f)?;
                if self.version>=2.399 {self.fields("LinkList:traits",path,&mut f)?;}
                self.fields("Where:str Appearance:str Security:str Umbra:str Notes:str LastModified:date",path,&mut f)?;
            },
            "Award" => self.fields("XP:bool Name:str ChangeType:i32 Change:f32 Reason:str",path,&mut f)?,
            "Template" => self.fields("Name:str IsCharacterSheet:bool TextFile:str RtfFile:str HtmlFile:str",path,&mut f)?,
            "Calendar" => {
                self.fields("LastModified:date",path,&mut f)?;
                f.insert("Entries".into(),Value::Array(self.array("CalendarEntry",&format!("{path}.Entries"))?));
            },
            "CalendarEntry" => self.fields("GameDate:date Time:str Place:str Notes:str",path,&mut f)?,
            "AprSettings" => {
                self.fields("PersonalActions:i16 AddCommon:bool",path,&mut f)?;
                if self.version>=2.397 {self.fields("CarryUnused:bool",path,&mut f)?;}
                self.fields("PublicRumors:bool PersonalRumors:bool RaceRumors:bool GroupRumors:bool SubgroupRumors:bool InfluenceRumors:bool PreviousRumors:bool CopyPrevious:bool BackgroundActions:traits ActionsPerLevel:traits",path,&mut f)?;
            },
            "Query" => {
                self.fields("Name:str Inventory:i32 MatchAll:bool SortKey:str SortDescend:bool LastModified:date",path,&mut f)?;
                f.insert("Clauses".into(),Value::Array(self.array("QueryClause",&format!("{path}.Clauses"))?));
            },
            "QueryClause" => self.fields("Key:str Comparison:i32 CompNot:bool Find:str Number:f64",path,&mut f)?,
            "Action" => {
                self.fields("ActDate:date CharName:str Done:bool LastModified:date",path,&mut f)?;
                f.insert("SubActions".into(),Value::Array(self.array("ActionNode",&format!("{path}.SubActions"))?));
            },
            "ActionNode" => self.fields("Name:str Level:i16 Unused:i16 Total:i16 Growth:i16 Effects:Effects Action:str Result:str",path,&mut f)?,
            "Effects" => { f.insert("Entries".into(),Value::Array(self.array("Effect",&format!("{path}.Entries"))?)); },
            "Effect" => self.fields("Target:i32 When:date Item:str Subitem:str",path,&mut f)?,
            "Plot" => {
                self.fields("Name:str StartDate:date EndDate:date",path,&mut f)?;
                if self.version>=2.399 {self.fields("Narrator:str",path,&mut f)?;}
                self.fields("LastModified:date Outline:str",path,&mut f)?;
                if self.version>=2.399 {self.fields("CastList:traits",path,&mut f)?;}
                f.insert("Developments".into(),Value::Array(self.array("PlotNode",&format!("{path}.Developments"))?));
            },
            "PlotNode" => self.fields("DevDate:date Effects:Effects Development:str",path,&mut f)?,
            "Rumor" => {
                self.fields("Title:str RumorDate:date Category:i32 MultiKey:str MultiMatch:str Done:bool LastModified:date",path,&mut f)?;
                if f["MultiKey"] == "" {self.fields("Query:Query",path,&mut f)?;}
                f.insert("SubRumors".into(),Value::Array(self.array("RumorNode",&format!("{path}.SubRumors"))?));
            },
            "RumorNode" => self.fields("Level:i16 Rumor:str",path,&mut f)?,
            "Character" => {
                let race=self.i16(&format!("{path}.RaceCode"))?;
                let profile=self.schema["versions"].as_object().unwrap().iter().find(|(v,_)| (v.parse::<f64>().unwrap()-self.version).abs()<1e-9)
                    .and_then(|(_,p)|p.get(race.to_string())).cloned()
                    .ok_or_else(||self.fail("UNKNOWN_CHARACTER_TYPE",path,format!("Unknown race code {race}; cannot safely skip an unsized record")))?;
                f.insert("RaceCode".into(),json!(race)); f.insert("CharacterType".into(),profile["name"].clone());
                for field in profile["fields"].as_array().unwrap() {
                    let key=field["name"].as_str().unwrap();
                    f.insert(key.into(),self.value(field["kind"].as_str().unwrap(),&format!("{path}.{key}"))?);
                }
            },
            _ => return Err(self.fail("INTERNAL_SCHEMA",path,format!("No reader for {kind}"))),
        }
        Ok(Value::Object(f))
    }
    fn section(&mut self, kind: &str, key: &str, report: &mut Report) -> ReadResult<()> {
        let count_start = self.pos;
        let n = match self.count(key) {
            Ok(n) => n,
            Err(e) => {
                self.pos = count_start;
                return Err(e);
            }
        };
        let list = report.sections.entry(key.into()).or_default();
        for i in 0..n {
            let start = self.pos;
            match self.record(kind,&format!("{key}[{i}]")) {
                Ok(fields)=>list.push(json!({"type":kind,"fields":fields,"source_span":{"start":start,"end":self.pos}})),
                Err(error)=>{self.pos=start; return Err(error);}
            }
        }
        Ok(())
    }
    fn singleton(&mut self, kind: &str, key: &str, report: &mut Report) -> ReadResult<()> {
        let start = self.pos;
        match self.record(kind, key) {
            Ok(fields) => {
                report.metadata.insert(key.into(), fields);
                Ok(())
            }
            Err(e) => {
                self.pos = start;
                Err(e)
            }
        }
    }
    fn body(&mut self, report: &mut Report, game: bool) -> ReadResult<()> {
        if game {
            let n = self.i16("DeclaredInventoryCount")?;
            if n < 0 {
                return Err(self.fail(
                    "NEGATIVE_COUNT",
                    "DeclaredInventoryCount",
                    "Negative inventory count",
                ));
            }
            report
                .metadata
                .insert("DeclaredInventoryCount".into(), json!(n));
            let mut info = Map::new();
            self.fields("ChronicleTitle:str Website:str EMail:str Phone:str UsualTime:str UsualPlace:str Description:str ExtendedHealth:bool EnforceHistory:bool","Game",&mut info)?;
            if self.version == 2.396 {
                self.fields("LegacyReservedFlag:bool", "Game", &mut info)?;
            }
            if self.version >= 2.397 {
                self.fields(
                    "STCommentStart:str STCommentEnd:str LinkTraitMaxes:bool",
                    "Game",
                    &mut info,
                )?;
            }
            if self.version >= 2.399 {
                self.fields("RandomTraits:str", "Game", &mut info)?;
            }
            self.fields("MenuFileName:str", "Game", &mut info)?;
            report.metadata.insert("Game".into(), Value::Object(info));
            self.singleton("Calendar", "Calendar", report)?;
            if self.version >= 2.397 {
                self.section("Award", "awards", report)?;
                self.section("Template", "templates", report)?;
            }
            self.singleton("AprSettings", "AprSettings", report)?;
        } else {
            let flag = self.i16("CalendarPresent")?;
            if flag != 0 && flag != 1 {
                return Err(self.fail("INVALID_FLAG", "CalendarPresent", "Expected 0 or 1"));
            }
            if flag == 1 {
                self.singleton("Calendar", "Calendar", report)?;
            }
            if self.version >= 2.397 {
                let flag = self.i16("AprSettingsPresent")?;
                if flag != 0 && flag != 1 {
                    return Err(self.fail("INVALID_FLAG", "AprSettingsPresent", "Expected 0 or 1"));
                }
                if flag == 1 {
                    self.singleton("AprSettings", "AprSettings", report)?;
                }
                self.section("Award", "awards", report)?;
                self.section("Template", "templates", report)?;
            }
        }
        for (kind, key) in [
            ("Player", "players"),
            ("Character", "characters"),
            ("Query", "queries"),
            ("Item", "items"),
            ("Rote", "rotes"),
            ("Location", "locations"),
            ("Action", "actions"),
            ("Plot", "plots"),
            ("Rumor", "rumors"),
        ] {
            self.section(kind, key, report)?;
        }
        Ok(())
    }
}

/// Parse bytes without executing templates, opening referenced files, or modifying sources.
/// `encoding_label` is the original Windows ANSI code page for binary/undeclared XML.
pub fn inspect(bytes: &[u8], encoding_label: &str) -> Report {
    let encoding = Encoding::for_label(encoding_label.as_bytes()).unwrap_or(WINDOWS_1252);
    let mut report = Report::new(bytes, encoding.name());
    if bytes.len() > MAX_INPUT_BYTES {
        report.note(
            "error",
            "FILE_LIMIT",
            "File exceeds 32 MiB inspection limit",
            None,
            "input",
        );
        return report;
    }
    if Encoding::for_label(encoding_label.as_bytes()).is_none() {
        report.note(
            "error",
            "UNKNOWN_ENCODING",
            format!("Unknown encoding {encoding_label}"),
            None,
            "input",
        );
        return report;
    }
    let xml_start = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if bytes.starts_with(&[0xff, 0xfe])
        || bytes.starts_with(&[0xfe, 0xff])
        || xml_start.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'<')
    {
        inspect_xml(bytes, encoding, &mut report);
        return report;
    }
    let mut r = Reader {
        bytes,
        pos: 0,
        version: 0.0,
        encoding,
        objects: 0,
        non_ascii: vec![],
        diagnostics: vec![],
        schema: &WIRE_SCHEMA,
    };
    let outcome = (|| -> ReadResult<()> {
        let header = r.string("Header")?;
        let game = match header.as_str() {
            "GVBG" => true,
            "GVBE" => false,
            _ => {
                return Err(r.fail(
                    "UNSUPPORTED_FORMAT",
                    "Header",
                    format!("Unsupported signature {header:?}; expected GVBG or GVBE"),
                ));
            }
        };
        report.format = if game { "gv3-binary" } else { "gex-binary" }.into();
        r.version = r.f64("Version")?;
        report.version = Some(r.version);
        if ![2.396, 2.397, 2.398, 2.399, 2.4, 3.0].contains(&r.version) {
            return Err(r.fail("UNSUPPORTED_VERSION","Version",format!("Unsupported binary version {}; supported layouts: 2.396, 2.397, 2.398, 2.399, 2.4, 3.0",r.version)));
        }
        report.note("info","CODEPAGE_ASSUMPTION","Binary strings have no encoding label. Windows-1252 is the default; select the original code page if different. Non-ASCII bytes are retained separately.",None,"encoding");
        r.body(&mut report, game)?;
        if r.pos != bytes.len() {
            return Err(r.fail(
                "TRAILING_BYTES",
                "input",
                format!("{} unparsed trailing bytes", bytes.len() - r.pos),
            ));
        }
        if game {
            let actual = report.record_count()
                - report.sections.get("awards").map_or(0, Vec::len)
                - report.sections.get("templates").map_or(0, Vec::len);
            if report.metadata["DeclaredInventoryCount"].as_u64() != Some(actual as u64) {
                report.note(
                    "warning",
                    "INVENTORY_COUNT_MISMATCH",
                    format!(
                        "Declared inventory count differs from {actual} parsed inventory records"
                    ),
                    None,
                    "DeclaredInventoryCount",
                );
            }
        }
        Ok(())
    })();
    report.non_ascii_strings = r.non_ascii;
    report.diagnostics.extend(r.diagnostics);
    match outcome {
        Ok(()) => report.status = "complete".into(),
        Err(e) => {
            report.status = if report.record_count() > 0 {
                "partial"
            } else if e.code.starts_with("UNSUPPORTED") || e.code == "UNKNOWN_CHARACTER_TYPE" {
                "unsupported"
            } else {
                "invalid"
            }
            .into();
            report.note("error", e.code, e.message, Some(e.offset), &e.path);
            // If a metadata/header read failed, retain the complete source bytes.
            let start = if report.record_count() == 0 { 0 } else { r.pos };
            report.unparsed = Some(
                json!({"offset":start,"length":bytes.len()-start,"bytes_hex":hex(&bytes[start..])}),
            );
        }
    }
    report
}

fn inspect_xml(bytes: &[u8], fallback: &'static Encoding, report: &mut Report) {
    let encoding = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        encoding_rs::UTF_8
    } else if bytes.starts_with(&[0xff, 0xfe]) {
        encoding_rs::UTF_16LE
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        encoding_rs::UTF_16BE
    } else {
        // XML's explicit encoding takes priority. Legacy Grapevine omits one and
        // writes Windows ANSI bytes, so use the caller's selected fallback then.
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
        if prefix.trim_start().starts_with("<?xml") {
            let declaration = prefix.split("?>").next().unwrap_or("");
            if let Some((_, rest)) = declaration.split_once("encoding") {
                let value = rest
                    .trim_start()
                    .strip_prefix('=')
                    .unwrap_or("")
                    .trim_start();
                let quote = value.chars().next().unwrap_or(' ');
                let label = value
                    .get(1..)
                    .unwrap_or("")
                    .split(quote)
                    .next()
                    .unwrap_or("");
                match Encoding::for_label(label.as_bytes()) {
                    Some(e) => e,
                    None => {
                        report.note(
                            "error",
                            "UNKNOWN_XML_ENCODING",
                            format!("Unknown XML encoding {label:?}"),
                            None,
                            "xml",
                        );
                        return;
                    }
                }
            } else {
                fallback
            }
        } else {
            fallback
        }
    };
    report.encoding = encoding.name().into();
    let (text, _, errors) = encoding.decode(bytes);
    if errors {
        report.note(
            "error",
            "TEXT_DECODE_ERROR",
            "XML decoding failed",
            None,
            "xml",
        );
        return;
    }
    report.raw_xml = Some(text.to_string());
    report.format = "grapevine-xml".into();
    let doc = match roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            nodes_limit: MAX_OBJECTS as u32,
            ..Default::default()
        },
    ) {
        Ok(d) => d,
        Err(roxmltree::Error::DtdDetected) => {
            report.note(
                "error",
                "XML_DTD_REJECTED",
                "DTDs and external entities are not supported",
                None,
                "xml",
            );
            return;
        }
        Err(e) => {
            report.note("error", "INVALID_XML", e.to_string(), None, "xml");
            return;
        }
    };
    let root = doc.root_element();
    if root
        .descendants()
        .any(|n| n.ancestors().take(66).count() > 64)
    {
        report.note(
            "error",
            "XML_DEPTH_LIMIT",
            "XML exceeds 64 nested levels",
            None,
            "xml",
        );
        return;
    }
    if root.tag_name().name() != "grapevine" || root.tag_name().namespace().is_some() {
        report.note(
            "error",
            "UNSUPPORTED_XML_ROOT",
            "Expected unnamespaced grapevine root",
            None,
            "xml",
        );
        report.status = "unsupported".into();
        return;
    }
    report.version = root
        .attribute("version")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite());
    if report.version.is_none() {
        report.note(
            "error",
            "INVALID_XML_VERSION",
            "Missing or invalid grapevine version",
            None,
            "xml",
        );
        return;
    }
    report
        .metadata
        .insert("XmlRootAttributes".into(), attributes(root));
    let mut unknown = false;
    for node in root.children().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        let section = match tag {
            "player" => "players",
            "vampire" | "werewolf" | "mortal" | "changeling" | "wraith" | "mage" | "fera"
            | "various" | "mummy" | "kueijin" | "hunter" | "demon" => "characters",
            "query" => "queries",
            "item" => "items",
            "rote" => "rotes",
            "location" => "locations",
            "action" => "actions",
            "plot" => "plots",
            "rumor" => "rumors",
            "award" => "awards",
            "template" => "templates",
            "calendar" | "aprsettings" | "usualplace" | "description" => {
                report.metadata.insert(tag.into(), xml_node(node));
                continue;
            }
            _ => {
                unknown = true;
                report.note(
                    "warning",
                    "UNKNOWN_XML_RECORD",
                    format!("Unrecognized root record {tag}; full XML subtree retained"),
                    Some(node.range().start),
                    tag,
                );
                "unmapped"
            }
        };
        if node.tag_name().namespace().is_some() {
            unknown = true;
            report.note(
                "warning",
                "XML_NAMESPACE",
                "Namespaced record preserved without semantic mapping",
                Some(node.range().start),
                tag,
            );
        }
        report.sections.entry(section.into()).or_default().push(json!({"type":tag,"xml":xml_node(node),"source_span":{"start":node.range().start,"end":node.range().end,"unit":"decoded_utf8_bytes"}}));
    }
    report.note("info","XML_PRESERVATION","All XML content is retained in raw_xml; record subtrees retain attributes and text. This is a lossless inspection representation, not normalized application import. No referenced paths are opened.",None,"xml");
    report.status = if unknown { "partial" } else { "complete" }.into();
}

fn attributes(node: roxmltree::Node<'_, '_>) -> Value {
    Value::Array(
        node.attributes()
            .map(|a| json!({"name":a.name(),"namespace":a.namespace(),"value":a.value()}))
            .collect(),
    )
}

fn xml_node(node: roxmltree::Node<'_, '_>) -> Value {
    let children=node.children().map(|n| {
        if n.is_element() {xml_node(n)}
        else if n.is_comment() {json!({"comment":n.text().unwrap_or("")})}
        else if n.is_pi() {json!({"processing_instruction":n.pi().map(|p|json!({"target":p.target,"value":p.value}))})}
        else {json!({"text":n.text().unwrap_or("")})}
    }).collect::<Vec<_>>();
    json!({"tag":node.tag_name().name(),"namespace":node.tag_name().namespace(),"attributes":attributes(node),"children":children})
}
