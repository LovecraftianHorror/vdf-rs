use std::fmt::{self, Write};

use crate::{error::Error, Obj, PartialVdf, Value, Vdf};

fn multiple_char(c: char, amount: usize) -> String {
    std::iter::repeat(c).take(amount).collect()
}

#[derive(Debug, Clone, Copy)]
enum RenderType {
    Escaped,
    Raw,
}

fn find_invalid_raw_char(s: &str) -> Option<char> {
    s.chars().find(|&c| c == '"').to_owned()
}

fn write_str(writer: &mut impl Write, s: &str, render_type: RenderType) -> fmt::Result {
    writer.write_char('"')?;

    match render_type {
        RenderType::Escaped => {
            for c in s.chars() {
                match c {
                    '\n' => writer.write_str(r"\n"),
                    '\r' => writer.write_str(r"\r"),
                    '\t' => writer.write_str(r"\t"),
                    '\"' => writer.write_str(r#"\""#),
                    '\\' => writer.write_str(r"\\"),
                    reg => writer.write_char(reg),
                }?
            }
        }
        RenderType::Raw => writer.write_str(s)?,
    }

    writer.write_char('"')
}

fn write_pair<'a>(
    writer: &mut impl Write,
    num_indents: usize,
    key: &str,
    value: &Value<'a>,
    render_type: RenderType,
) -> fmt::Result {
    // Write the indented key
    writer.write_str(&multiple_char('\t', num_indents))?;
    write_str(writer, key, render_type)?;

    // Followed by the value
    if value.is_str() {
        writer.write_char('\t')?;
    } else {
        writer.write_char('\n')?;
    }
    value.write_indented(writer, num_indents, render_type)?;

    writer.write_char('\n')
}

fn write_obj<'a>(
    writer: &mut impl Write,
    num_indents: usize,
    obj: &Obj<'a>,
    render_type: RenderType,
) -> fmt::Result {
    for (key, values) in obj.iter() {
        for value in values {
            write_pair(writer, num_indents, key, value, render_type)?;
        }
    }

    Ok(())
}

impl<'a> fmt::Display for PartialVdf<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: does this handle rendering bases correctly? Are there invalid characters in bases?
        for base in &self.bases {
            writeln!(f, "#base \"{}\"", base)?;
        }

        if !self.bases.is_empty() {
            f.write_char('\n')?;
        }

        write_pair(f, 0, &self.key, &self.value, RenderType::Escaped)
    }
}

impl<'a> fmt::Display for Vdf<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_indented(f, 0, RenderType::Escaped)
    }
}

impl<'a> Vdf<'a> {
    pub fn render(&self, writer: &mut impl Write) -> crate::error::Result<()> {
        write!(writer, "{}", self).map_err(Into::into)
    }

    pub fn render_raw(&self, writer: &mut impl Write) -> crate::error::Result<()> {
        match self.find_invalid_raw_char() {
            Some(invalid_char) => Err(Error::RawRenderError { invalid_char }),
            None => self
                .write_indented(writer, 0, RenderType::Raw)
                .map_err(Into::into),
        }
    }

    fn find_invalid_raw_char(&self) -> Option<char> {
        find_invalid_raw_char(&self.key).or_else(|| self.value.find_invalid_raw_char())
    }

    fn write_indented(
        &self,
        writer: &mut impl Write,
        num_indents: usize,
        render_type: RenderType,
    ) -> fmt::Result {
        write_pair(writer, num_indents, &self.key, &self.value, render_type)
    }
}

impl<'a> fmt::Display for Value<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_indented(f, 0, RenderType::Escaped)
    }
}

impl<'a> Value<'a> {
    fn write_indented(
        &self,
        writer: &mut impl Write,
        num_indents: usize,
        render_type: RenderType,
    ) -> fmt::Result {
        // Only `Obj` gets indented
        match self {
            Value::Str(s) => write_str(writer, s, render_type),
            Value::Obj(obj) => {
                writeln!(writer, "{}{{", multiple_char('\t', num_indents))?;
                write_obj(writer, num_indents + 1, obj, render_type)?;
                // obj.write_indented(f, num_indents + 1)?;
                write!(writer, "{}}}", multiple_char('\t', num_indents))
            }
        }
    }

    fn find_invalid_raw_char(&self) -> Option<char> {
        match self {
            Self::Str(s) => find_invalid_raw_char(s),
            Self::Obj(obj) => {
                for (key, values) in obj.iter() {
                    let maybe_c = find_invalid_raw_char(key).or_else(|| {
                        values
                            .iter()
                            .find_map(|value| value.find_invalid_raw_char())
                    });

                    if maybe_c.is_some() {
                        return maybe_c;
                    }
                }

                None
            }
        }
    }
}
