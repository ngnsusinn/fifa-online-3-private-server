use byteorder::{ByteOrder, LittleEndian};
use fo3_server::tools::eadb::EaTable;
use std::env;
use std::fs;

struct DbTableInfo {
    name: String,
    offset: usize,
    raw_slice: (usize, usize),
}

fn list_db_tables(data: &[u8]) -> Result<Vec<DbTableInfo>, String> {
    if data.len() < 0x20 || !data.starts_with(b"DB\0\x08") {
        return Err("Not a valid EA DB file (magic mismatch)".to_string());
    }
    let n = LittleEndian::read_u32(&data[0x10..0x14]) as usize;
    let base = 0x18 + 8 * n + 4;
    if data.len() < base {
        return Err("DB file truncated before table directory end".to_string());
    }

    let mut names = Vec::new();
    let mut starts = Vec::new();

    for i in 0..n {
        let entry_off = 0x18 + 8 * i;
        let name_bytes = &data[entry_off..entry_off + 4];
        let name = String::from_utf8_lossy(name_bytes)
            .trim_end_matches('\0')
            .to_string();
        let rel_off = LittleEndian::read_u32(&data[entry_off + 4..entry_off + 8]) as usize;
        let abs_start = base + rel_off;
        names.push(name);
        starts.push(abs_start);
    }
    starts.push(data.len());

    let mut result = Vec::new();
    for i in 0..n {
        let start = starts[i];
        let end = starts[i + 1];
        if start <= end && end <= data.len() {
            result.push(DbTableInfo {
                name: names[i].clone(),
                offset: start,
                raw_slice: (start, end),
            });
        }
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: eadb <file.db> [table_shortname]");
        std::process::exit(1);
    }

    let file_path = &args[1];
    let data = fs::read(file_path)?;
    let tables = list_db_tables(&data).map_err(|e| std::io::Error::other(e))?;

    if args.len() == 2 {
        println!("EA DB file: {} ({} tables)", file_path, tables.len());
        println!("{:<8} {:<12} {:<10} {:<10}", "Name", "Offset", "Records", "RowSize");
        println!("{}", "-".repeat(44));
        for t in &tables {
            let slice = &data[t.raw_slice.0..t.raw_slice.1];
            if let Ok(parsed) = EaTable::parse(slice) {
                println!(
                    "{:<8} {:#010x}   {:<10} {:<10}",
                    t.name,
                    t.offset,
                    parsed.rows.len(),
                    parsed.record_size
                );
            } else {
                println!("{:<8} {:#010x}   (failed to parse)", t.name, t.offset);
            }
        }
        return Ok(());
    }

    let target_table = &args[2];
    let table_info = tables
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(target_table))
        .ok_or_else(|| format!("Table '{}' not found in database", target_table))?;

    let slice = &data[table_info.raw_slice.0..table_info.raw_slice.1];
    let parsed = EaTable::parse(slice).map_err(|e| std::io::Error::other(e))?;

    println!("Table: {} (at {:#010x})", table_info.name, table_info.offset);
    println!("Record size: {} bytes, Rows: {}", parsed.record_size, parsed.rows.len());
    println!("\nFields ({}):", parsed.fields.len());
    println!("{:<8} {:<8} {:<12} {:<10}", "Name", "Type", "BitOffset", "BitDepth");
    println!("{}", "-".repeat(40));
    let mut fields: Vec<_> = parsed.fields.values().collect();
    fields.sort_by_key(|f| f.bit_offset);
    for f in fields {
        let type_str = match f.field_type {
            0 => "string",
            3 => "int",
            4 => "float",
            t => &format!("type_{}", t),
        };
        println!("{:<8} {:<8} {:<12} {:<10}", f.short_name, type_str, f.bit_offset, f.bit_depth);
    }

    if !parsed.rows.is_empty() {
        println!("\nFirst row sample:");
        let first_row = &parsed.rows[0];
        for (name, desc) in &parsed.fields {
            if desc.field_type == 0 {
                if let Some(s) = parsed.get_string(first_row, name) {
                    println!("  {}: {:?}", name, s);
                }
            } else if let Some(n) = parsed.get_int(first_row, name) {
                println!("  {}: {}", name, n);
            }
        }
    }

    Ok(())
}
