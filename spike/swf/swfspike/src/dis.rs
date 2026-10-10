//! AS2 disassembly: every DoAction / DoInitAction, functions nested.
use swf::avm1::read::Reader;
use swf::avm1::types::{Action, Value};
use swf::{SwfStr, Tag};

pub fn run(data: &[u8]) -> anyhow::Result<()> {
    let buf = swf::decompress_swf(data)?;
    let swf = swf::parse_swf(&buf)?;
    let v = swf.header.version();
    let mut names = std::collections::HashMap::new();
    collect_exports(&swf.tags, &mut names);
    walk(&swf.tags, v, "root", &names);
    Ok(())
}

fn collect_exports(tags: &[Tag], names: &mut std::collections::HashMap<u16, String>) {
    for t in tags {
        if let Tag::ExportAssets(a) = t {
            for e in a {
                names.insert(e.id, e.name.to_str_lossy(swf::UTF_8).into());
            }
        }
    }
}

fn walk(tags: &[Tag], v: u8, owner: &str, names: &std::collections::HashMap<u16, String>) {
    for (frame, t) in tags.iter().enumerate() {
        match t {
            Tag::DoAction(d) => {
                println!("== DoAction in {owner} (tag {frame})");
                dis(d, v, 1, &mut Vec::new());
            }
            Tag::DoInitAction { id, action_data } => {
                let n = names.get(id).cloned().unwrap_or_default();
                println!("== DoInitAction sprite {id} {n}");
                dis(action_data, v, 1, &mut Vec::new());
            }
            Tag::DefineSprite(s) => {
                let n = names.get(&s.id).cloned().unwrap_or_default();
                walk(&s.tags, v, &format!("sprite {} {n}", s.id), names);
            }
            _ => {}
        }
    }
}

fn s(x: &SwfStr) -> String {
    x.to_str_lossy(swf::UTF_8).into_owned()
}

fn dis(data: &[u8], v: u8, depth: usize, pool: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    let mut r = Reader::new(data, v);
    loop {
        let off = data.len() - r.get_mut().len();
        let a = match r.read_action() {
            Ok(a) => a,
            Err(e) => {
                println!("{pad}!! {e:?}");
                return;
            }
        };
        match a {
            Action::End => return,
            Action::ConstantPool(p) => {
                *pool = p.strings.iter().map(|x| s(x)).collect();
                println!("{pad}{off:5} ConstantPool [{} strings]", pool.len());
            }
            Action::Push(p) => {
                let vals: Vec<String> = p
                    .values
                    .iter()
                    .map(|x| match x {
                        Value::Str(x) => format!("{:?}", s(x)),
                        Value::ConstantPool(i) => {
                            format!("{:?}", pool.get(*i as usize).cloned().unwrap_or_default())
                        }
                        Value::Register(i) => format!("r{i}"),
                        o => format!("{o:?}"),
                    })
                    .collect();
                println!("{pad}{off:5} Push {}", vals.join(", "));
            }
            Action::DefineFunction(f) => {
                let f: swf::avm1::types::DefineFunction2 = f.into();
                fun(&f, v, depth, pool, off);
            }
            Action::DefineFunction2(f) => fun(&f, v, depth, pool, off),
            Action::Try(t) => {
                println!("{pad}{off:5} Try");
                dis(t.try_body, v, depth + 1, pool);
                if let Some((_, b)) = t.catch_body {
                    println!("{pad}      Catch");
                    dis(b, v, depth + 1, pool);
                }
                if let Some(b) = t.finally_body {
                    println!("{pad}      Finally");
                    dis(b, v, depth + 1, pool);
                }
            }
            Action::With(w) => {
                println!("{pad}{off:5} With");
                dis(w.actions, v, depth + 1, pool);
            }
            o => println!("{pad}{off:5} {o:?}"),
        }
    }
}

fn fun(
    f: &swf::avm1::types::DefineFunction2,
    v: u8,
    depth: usize,
    pool: &mut Vec<String>,
    off: usize,
) {
    let pad = "  ".repeat(depth);
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| match p.register_index {
            Some(r) => format!("{}=r{}", s(p.name), r.get()),
            None => s(p.name),
        })
        .collect();
    println!(
        "{pad}{off:5} function {}({}) {{",
        s(f.name),
        params.join(", ")
    );
    let mut inner = pool.clone();
    dis(f.actions, v, depth + 1, &mut inner);
    println!("{pad}      }}");
}
