//! Lists fonts, imports and exports in a SWF.
use swf::Tag;

pub fn run(data: &[u8]) -> anyhow::Result<()> {
    let buf = swf::decompress_swf(data)?;
    let swf = swf::parse_swf(&buf)?;
    for t in &swf.tags {
        match t {
            Tag::DefineFont2(f) => println!(
                "font id {} {:?} v{} glyphs {} flags {:?}",
                f.id,
                f.name.to_str_lossy(swf::UTF_8),
                f.version,
                f.glyphs.len(),
                f.flags
            ),
            Tag::ExportAssets(a) => {
                for e in a {
                    println!("export {} {:?}", e.id, e.name.to_str_lossy(swf::UTF_8));
                }
            }
            Tag::ImportAssets { url, imports } => {
                for e in imports {
                    println!(
                        "import {:?} {} {:?}",
                        url.to_str_lossy(swf::UTF_8),
                        e.id,
                        e.name.to_str_lossy(swf::UTF_8)
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}
