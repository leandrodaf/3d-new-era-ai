//! Reader for the Java Object Serialization Stream Protocol (version 5).
//!
//! Produces a graph of objects addressed by handle, keeping every field and
//! every custom `writeObject` annotation, so callers can decode any class
//! without this module knowing about it.

use std::collections::HashMap;

const STREAM_MAGIC: u16 = 0xACED;
const STREAM_VERSION: u16 = 5;
const BASE_HANDLE: u32 = 0x007E_0000;

const TC_NULL: u8 = 0x70;
const TC_REFERENCE: u8 = 0x71;
const TC_CLASSDESC: u8 = 0x72;
const TC_OBJECT: u8 = 0x73;
const TC_STRING: u8 = 0x74;
const TC_ARRAY: u8 = 0x75;
const TC_CLASS: u8 = 0x76;
const TC_BLOCKDATA: u8 = 0x77;
const TC_ENDBLOCKDATA: u8 = 0x78;
const TC_RESET: u8 = 0x79;
const TC_BLOCKDATALONG: u8 = 0x7A;
const TC_EXCEPTION: u8 = 0x7B;
const TC_LONGSTRING: u8 = 0x7C;
const TC_PROXYCLASSDESC: u8 = 0x7D;
const TC_ENUM: u8 = 0x7E;

const SC_WRITE_METHOD: u8 = 0x01;
const SC_SERIALIZABLE: u8 = 0x02;
const SC_EXTERNALIZABLE: u8 = 0x04;
const SC_BLOCK_DATA: u8 = 0x08;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a Java serialization stream")]
    BadMagic,
    #[error("unexpected end of stream")]
    Eof,
    #[error("unexpected type code 0x{code:02x} at byte {at}")]
    UnexpectedCode { code: u8, at: usize },
    #[error("invalid handle 0x{0:x}")]
    BadHandle(u32),
    #[error("externalizable class `{0}` without block data cannot be read")]
    Externalizable(String),
    #[error("serialized exception in stream")]
    Exception,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Index of an entry in [`Graph::entries`].
pub type Handle = usize;

/// A field or array element value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// String, object, array, enum or class.
    Ref(Handle),
}

#[derive(Debug, Clone)]
pub struct FieldDesc {
    pub type_code: u8,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ClassDesc {
    pub name: String,
    pub flags: u8,
    pub fields: Vec<FieldDesc>,
    pub super_class: Option<Handle>,
}

/// Custom data written by a class's `writeObject`: primitive block data and
/// objects, in stream order.
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Block(Vec<u8>),
    Value(Value),
}

/// Data of one class in an object's hierarchy.
#[derive(Debug, Clone)]
pub struct ClassData {
    pub class: Handle,
    pub fields: Vec<(String, Value)>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub class: Handle,
    /// From the topmost serializable superclass down to the object's class.
    pub data: Vec<ClassData>,
}

#[derive(Debug, Clone)]
pub enum Entry {
    ClassDesc(ClassDesc),
    Object(Object),
    String(String),
    Array {
        class: Handle,
        values: Vec<Value>,
    },
    Enum {
        class: Handle,
        constant: String,
    },
    Class(Handle),
    /// Placeholder while an entry is being read (or for proxy classes).
    Pending,
}

/// Everything read from a stream.
#[derive(Debug, Default)]
pub struct Graph {
    pub entries: Vec<Entry>,
    /// Top-level contents, in order.
    pub roots: Vec<Value>,
}

impl Graph {
    pub fn entry(&self, value: &Value) -> Option<&Entry> {
        match value {
            Value::Ref(h) => self.entries.get(*h),
            _ => None,
        }
    }

    pub fn object(&self, value: &Value) -> Option<&Object> {
        match self.entry(value)? {
            Entry::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn string(&self, value: &Value) -> Option<&str> {
        match self.entry(value)? {
            Entry::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn class_desc(&self, handle: Handle) -> Option<&ClassDesc> {
        match self.entries.get(handle)? {
            Entry::ClassDesc(c) => Some(c),
            _ => None,
        }
    }

    /// Fully qualified class name of an object, array or enum value.
    pub fn class_name(&self, value: &Value) -> Option<&str> {
        let class = match self.entry(value)? {
            Entry::Object(o) => o.class,
            Entry::Array { class, .. } | Entry::Enum { class, .. } => *class,
            _ => return None,
        };
        self.class_desc(class).map(|c| c.name.as_str())
    }

    /// Whether `value` is an object of `class` or a subclass of it (by
    /// fully qualified name).
    pub fn is_instance(&self, value: &Value, class: &str) -> bool {
        self.object(value).is_some_and(|o| {
            o.data
                .iter()
                .any(|d| self.class_desc(d.class).is_some_and(|c| c.name == class))
        })
    }

    /// Reads a stream from bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader {
            bytes,
            pos: 0,
            graph: Self::default(),
            handles: HashMap::new(),
        };
        if reader.u16()? != STREAM_MAGIC || reader.u16()? != STREAM_VERSION {
            return Err(Error::BadMagic);
        }
        while reader.pos < bytes.len() {
            match reader.content()? {
                Content::Value(v) => reader.graph.roots.push(v),
                Content::Block(_) | Content::End => {}
            }
        }
        Ok(reader.graph)
    }
}

enum Content {
    Value(Value),
    Block(Vec<u8>),
    End,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    graph: Graph,
    /// Stream handle → entry index (handles restart after `TC_RESET`).
    handles: HashMap<u32, Handle>,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::Eof)?;
        let slice = self.bytes.get(self.pos..end).ok_or(Error::Eof)?;
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn peek(&self) -> Result<u8> {
        self.bytes.get(self.pos).copied().ok_or(Error::Eof)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    fn utf(&mut self) -> Result<String> {
        let len = usize::from(self.u16()?);
        Ok(decode_modified_utf8(self.take(len)?))
    }

    fn long_utf(&mut self) -> Result<String> {
        let len = usize::try_from(self.i64()?).map_err(|_| Error::Eof)?;
        Ok(decode_modified_utf8(self.take(len)?))
    }

    /// Reserves the next stream handle for a new entry.
    fn new_handle(&mut self) -> Handle {
        let index = self.graph.entries.len();
        self.graph.entries.push(Entry::Pending);
        let stream =
            BASE_HANDLE + u32::try_from(self.handles.len()).unwrap_or(u32::MAX - BASE_HANDLE);
        self.handles.insert(stream, index);
        index
    }

    fn content(&mut self) -> Result<Content> {
        let at = self.pos;
        match self.peek()? {
            TC_BLOCKDATA => {
                self.pos += 1;
                let len = usize::from(self.u8()?);
                Ok(Content::Block(self.take(len)?.to_vec()))
            }
            TC_BLOCKDATALONG => {
                self.pos += 1;
                let len = usize::try_from(self.i32()?).map_err(|_| Error::Eof)?;
                Ok(Content::Block(self.take(len)?.to_vec()))
            }
            TC_ENDBLOCKDATA => {
                self.pos += 1;
                Ok(Content::End)
            }
            TC_RESET => {
                self.pos += 1;
                self.handles.clear();
                self.content()
            }
            TC_EXCEPTION => {
                let _ = at;
                Err(Error::Exception)
            }
            _ => self.object().map(Content::Value),
        }
    }

    /// Reads any object-like content: null, reference, string, object…
    fn object(&mut self) -> Result<Value> {
        let at = self.pos;
        let code = self.u8()?;
        match code {
            TC_NULL => Ok(Value::Null),
            TC_REFERENCE => {
                let handle = u32::try_from(self.i32()?).map_err(|_| Error::BadHandle(0))?;
                self.handles
                    .get(&handle)
                    .map(|h| Value::Ref(*h))
                    .ok_or(Error::BadHandle(handle))
            }
            TC_STRING => {
                let h = self.new_handle();
                let s = self.utf()?;
                self.graph.entries[h] = Entry::String(s);
                Ok(Value::Ref(h))
            }
            TC_LONGSTRING => {
                let h = self.new_handle();
                let s = self.long_utf()?;
                self.graph.entries[h] = Entry::String(s);
                Ok(Value::Ref(h))
            }
            TC_CLASSDESC | TC_PROXYCLASSDESC => {
                self.pos = at;
                Ok(self.class_desc()?.map_or(Value::Null, Value::Ref))
            }
            TC_CLASS => {
                let desc = self.class_desc()?;
                let h = self.new_handle();
                self.graph.entries[h] = desc.map_or(Entry::Pending, Entry::Class);
                Ok(Value::Ref(h))
            }
            TC_ENUM => {
                let class = self
                    .class_desc()?
                    .ok_or(Error::UnexpectedCode { code, at })?;
                let h = self.new_handle();
                let name = self.object()?;
                let constant = self.graph.string(&name).unwrap_or_default().to_owned();
                self.graph.entries[h] = Entry::Enum { class, constant };
                Ok(Value::Ref(h))
            }
            TC_ARRAY => {
                let class = self
                    .class_desc()?
                    .ok_or(Error::UnexpectedCode { code, at })?;
                let h = self.new_handle();
                let len = usize::try_from(self.i32()?).map_err(|_| Error::Eof)?;
                let element = self
                    .graph
                    .class_desc(class)
                    .and_then(|c| c.name.as_bytes().get(1).copied())
                    .unwrap_or(b'L');
                let mut values = Vec::with_capacity(len.min(1 << 20));
                for _ in 0..len {
                    values.push(self.value(element)?);
                }
                self.graph.entries[h] = Entry::Array { class, values };
                Ok(Value::Ref(h))
            }
            TC_OBJECT => {
                let class = self
                    .class_desc()?
                    .ok_or(Error::UnexpectedCode { code, at })?;
                let h = self.new_handle();
                let mut chain = Vec::new();
                let mut next = Some(class);
                while let Some(c) = next {
                    chain.push(c);
                    next = self.graph.class_desc(c).and_then(|d| d.super_class);
                }
                chain.reverse();
                let mut data = Vec::with_capacity(chain.len());
                for c in chain {
                    let desc = self
                        .graph
                        .class_desc(c)
                        .cloned()
                        .ok_or(Error::BadHandle(0))?;
                    let mut class_data = ClassData {
                        class: c,
                        fields: Vec::new(),
                        annotations: Vec::new(),
                    };
                    if desc.flags & SC_EXTERNALIZABLE != 0 {
                        if desc.flags & SC_BLOCK_DATA == 0 {
                            return Err(Error::Externalizable(desc.name));
                        }
                        class_data.annotations = self.annotations()?;
                    } else if desc.flags & SC_SERIALIZABLE != 0 {
                        for field in &desc.fields {
                            let value = self.value(field.type_code)?;
                            class_data.fields.push((field.name.clone(), value));
                        }
                        if desc.flags & SC_WRITE_METHOD != 0 {
                            class_data.annotations = self.annotations()?;
                        }
                    }
                    data.push(class_data);
                }
                self.graph.entries[h] = Entry::Object(Object { class, data });
                Ok(Value::Ref(h))
            }
            _ => Err(Error::UnexpectedCode { code, at }),
        }
    }

    /// Contents until `TC_ENDBLOCKDATA`.
    fn annotations(&mut self) -> Result<Vec<Annotation>> {
        let mut out = Vec::new();
        loop {
            match self.content()? {
                Content::End => return Ok(out),
                Content::Block(bytes) => out.push(Annotation::Block(bytes)),
                Content::Value(v) => out.push(Annotation::Value(v)),
            }
        }
    }

    /// Reads a class descriptor (new, proxy, reference or null) and returns
    /// its entry handle.
    fn class_desc(&mut self) -> Result<Option<Handle>> {
        let at = self.pos;
        let code = self.u8()?;
        match code {
            TC_NULL => Ok(None),
            TC_REFERENCE => {
                let handle = u32::try_from(self.i32()?).map_err(|_| Error::BadHandle(0))?;
                self.handles
                    .get(&handle)
                    .copied()
                    .map(Some)
                    .ok_or(Error::BadHandle(handle))
            }
            TC_CLASSDESC => {
                let name = self.utf()?;
                let _uid = self.i64()?;
                let h = self.new_handle();
                let flags = self.u8()?;
                let count = self.u16()?;
                let mut fields = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let type_code = self.u8()?;
                    let name = self.utf()?;
                    if type_code == b'L' || type_code == b'[' {
                        // Field class name, as a string object.
                        self.object()?;
                    }
                    fields.push(FieldDesc { type_code, name });
                }
                self.annotations()?;
                let super_class = self.class_desc()?;
                self.graph.entries[h] = Entry::ClassDesc(ClassDesc {
                    name,
                    flags,
                    fields,
                    super_class,
                });
                Ok(Some(h))
            }
            TC_PROXYCLASSDESC => {
                let h = self.new_handle();
                let count = self.i32()?;
                for _ in 0..count {
                    self.utf()?;
                }
                self.annotations()?;
                let super_class = self.class_desc()?;
                self.graph.entries[h] = Entry::ClassDesc(ClassDesc {
                    name: "<proxy>".into(),
                    flags: SC_SERIALIZABLE,
                    fields: Vec::new(),
                    super_class,
                });
                Ok(Some(h))
            }
            _ => Err(Error::UnexpectedCode { code, at }),
        }
    }

    fn value(&mut self, type_code: u8) -> Result<Value> {
        Ok(match type_code {
            b'B' => Value::Byte(i8::from_be_bytes(self.array()?)),
            b'C' => Value::Char(self.u16()?),
            b'D' => Value::Double(f64::from_be_bytes(self.array()?)),
            b'F' => Value::Float(f32::from_be_bytes(self.array()?)),
            b'I' => Value::Int(self.i32()?),
            b'J' => Value::Long(self.i64()?),
            b'S' => Value::Short(i16::from_be_bytes(self.array()?)),
            b'Z' => Value::Bool(self.u8()? != 0),
            _ => self.object()?,
        })
    }
}

/// Java's modified UTF-8: `NUL` as two bytes and supplementary characters as
/// surrogate pairs (each encoded in three bytes).
fn decode_modified_utf8(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let cont = |k: usize| u16::from(bytes.get(i + k).copied().unwrap_or(0x80) & 0x3F);
        if b & 0x80 == 0 {
            units.push(u16::from(b));
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            units.push((u16::from(b & 0x1F) << 6) | cont(1));
            i += 2;
        } else {
            units.push((u16::from(b & 0x0F) << 12) | (cont(1) << 6) | cont(2));
            i += 3;
        }
    }
    String::from_utf16_lossy(&units)
}

/// A cursor over the block data of annotations, skipping interleaved objects.
#[derive(Debug)]
pub struct BlockReader<'a> {
    items: std::slice::Iter<'a, Annotation>,
    current: &'a [u8],
}

impl<'a> BlockReader<'a> {
    pub fn new(annotations: &'a [Annotation]) -> Self {
        Self {
            items: annotations.iter(),
            current: &[],
        }
    }

    fn fill(&mut self) -> bool {
        while self.current.is_empty() {
            match self.items.next() {
                Some(Annotation::Block(bytes)) => self.current = bytes,
                Some(Annotation::Value(_)) => {}
                None => return false,
            }
        }
        true
    }

    pub fn i32(&mut self) -> Option<i32> {
        let mut out = [0u8; 4];
        for byte in &mut out {
            if !self.fill() {
                return None;
            }
            *byte = self.current[0];
            self.current = &self.current[1..];
        }
        Some(i32::from_be_bytes(out))
    }
}

/// The objects written in annotations, in order.
pub fn annotation_values(annotations: &[Annotation]) -> impl Iterator<Item = &Value> {
    annotations.iter().filter_map(|a| match a {
        Annotation::Value(v) => Some(v),
        Annotation::Block(_) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-built stream: an object of class `demo.Point` (fields `int x`,
    /// `String label`, write method with an int block and a back-reference).
    fn sample() -> Vec<u8> {
        let mut s = vec![0xAC, 0xED, 0x00, 0x05];
        s.push(TC_OBJECT);
        s.push(TC_CLASSDESC);
        let name = b"demo.Point";
        s.extend(u16::try_from(name.len()).unwrap().to_be_bytes());
        s.extend(name);
        s.extend(1i64.to_be_bytes());
        s.push(SC_SERIALIZABLE | SC_WRITE_METHOD);
        s.extend(2u16.to_be_bytes());
        s.push(b'I');
        s.extend(1u16.to_be_bytes());
        s.push(b'x');
        s.push(b'L');
        s.extend(5u16.to_be_bytes());
        s.extend(b"label");
        s.push(TC_STRING);
        let field_class = b"Ljava/lang/String;";
        s.extend(u16::try_from(field_class.len()).unwrap().to_be_bytes());
        s.extend(field_class);
        s.push(TC_ENDBLOCKDATA);
        s.push(TC_NULL); // no superclass
        // Handles: 0 classdesc, 1 field class string, 2 object.
        s.extend(42i32.to_be_bytes());
        s.push(TC_STRING);
        // "São" in modified UTF-8.
        s.extend(4u16.to_be_bytes());
        s.extend([b'S', 0xC3, 0xA3, b'o']);
        // Annotation: block with an int, then a reference to the label string (handle 3).
        s.push(TC_BLOCKDATA);
        s.push(4);
        s.extend(7i32.to_be_bytes());
        s.push(TC_REFERENCE);
        s.extend((BASE_HANDLE + 3).to_be_bytes());
        s.push(TC_ENDBLOCKDATA);
        s
    }

    #[test]
    fn reads_objects_fields_annotations_and_references() {
        let graph = Graph::parse(&sample()).unwrap();
        let root = &graph.roots[0];
        assert_eq!(graph.class_name(root), Some("demo.Point"));
        let object = graph.object(root).unwrap();
        let data = &object.data[0];
        assert_eq!(data.fields[0], ("x".into(), Value::Int(42)));
        assert_eq!(graph.string(&data.fields[1].1), Some("São"));
        assert_eq!(BlockReader::new(&data.annotations).i32(), Some(7));
        let reference = annotation_values(&data.annotations).next().unwrap();
        assert_eq!(
            reference, &data.fields[1].1,
            "back-reference resolves to the same string"
        );
    }

    #[test]
    fn rejects_other_data() {
        assert!(matches!(Graph::parse(b"PK\x03\x04"), Err(Error::BadMagic)));
        assert!(Graph::parse(&sample()[..20]).is_err());
    }
}

/// Parses the `Home` entry of a real project given by `NEWERA_SH3D_SAMPLE`.
#[cfg(test)]
mod sample_tests {
    use std::io::Read;

    use super::*;

    #[test]
    #[ignore = "needs NEWERA_SH3D_SAMPLE=/path/to/file.sh3d"]
    fn parses_a_real_home() {
        let path = std::env::var("NEWERA_SH3D_SAMPLE").expect("NEWERA_SH3D_SAMPLE");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name("Home")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let graph = Graph::parse(&bytes).unwrap();
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for entry in &graph.entries {
            if let Entry::Object(o) = entry {
                *counts
                    .entry(graph.class_desc(o.class).unwrap().name.as_str())
                    .or_default() += 1;
            }
        }
        for (name, n) in &counts {
            println!("{n:5} {name}");
        }
        assert_eq!(
            graph.class_name(&graph.roots[0]),
            Some("com.eteks.sweethome3d.model.Home")
        );
    }
}
