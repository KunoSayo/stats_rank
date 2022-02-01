use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use clap::Parser;

/// Get stats from world and ranking them
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The server path
    #[clap(short, long, default_value = ".")]
    path: String,

    /// The stats key to rank
    #[clap(short, long)]
    key: String,

    /// Inverse the rank
    #[clap(short, long)]
    inverse: bool,

    /// The rank key is exactly
    #[clap(short, long)]
    exact: bool,

    /// Show uuid even the name was found
    #[clap(short, long)]
    show_uuid: bool,

    /// The rank limit for display
    #[clap(short, long, default_value_t = 9961)]
    limit: usize,
}

#[derive(Default, Debug)]
struct IdMap {
    /// Map uuid to name
    map: HashMap<String, String>,
}

impl IdMap {
    fn load_whitelist(&mut self, file_path: PathBuf) -> Result<(), Box<dyn Error>> {
        let mut file = File::open(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let json = json::parse(&content)?;
        for v in json.members() {
            let uuid = v["uuid"].as_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found uuid"))?;
            let name = v["name"].as_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found name"))?;
            if !self.map.contains_key(uuid) {
                self.map.insert(uuid.into(), name.into());
            }
        }

        Ok(())
    }

    fn load_user_name_cache(&mut self, file_path: PathBuf) -> Result<(), Box<dyn Error>> {
        let mut file = File::open(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let json = json::parse(&content)?;
        for (uuid, name_value) in json.entries() {
            self.map.insert(uuid.into(), name_value.as_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found name"))?.into());
        }
        Ok(())
    }

    fn load_user_name(&mut self, file_path: PathBuf) -> Result<(), Box<dyn Error>> {
        let mut file = File::open(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let json = json::parse(&content)?;
        for v in json.members() {
            let uuid = v["uuid"].as_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found uuid"))?;
            let name = v["name"].as_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found name"))?;
            self.map.insert(uuid.into(), name.into());
        }

        Ok(())
    }
}

fn get_level_name(file_path: PathBuf) -> Result<String, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    content.split("\n").filter(|x| x.starts_with("level-name"))
        .map(|x| x.split("=").skip(1).take(1).next())
        .map(|x| x.unwrap_or("world").trim().to_string()).next()
        .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot found level name"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = Args::parse();
    let mut id_map = IdMap::default();
    let dir = PathBuf::from(&args.path);
    if let Err(e) = id_map.load_whitelist(dir.join("whitelist.json")) {
        eprintln!("Load whitelist failed for {:?}", e);
    }

    if let Err(e) = id_map.load_user_name_cache(dir.join("usernamecache.json")) {
        eprintln!("Load user name cache failed for {:?}", e);
    }

    if let Err(e) = id_map.load_user_name(dir.join("usercache.json")) {
        eprintln!("Load user name failed for {:?}", e);
    }

    let world = match get_level_name(dir.join("server.properties")) {
        Ok(name) => {
            println!("Found world name: {}", &name);
            name
        },
        Err(e) => {
            eprintln!("Level name cannot be found for {:?}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "The level name cannot be found").into());
        }
    };
    let mut rank = HashMap::new();
    for x in dir.join(&world).join("stats").read_dir().expect("Read stats dir failed") {
        if let Err(e) = x {
            eprintln!("Load stat file failed for {:?}", e);
            continue;
        }
        let path = x.unwrap().path();
        if path.is_dir() {
            continue;
        }
        if let Some(uuid) = path.file_name()
            .map(|x| x.to_string_lossy().split('.').next().map(|x| x.to_string()))
            .flatten() {
            let mut file = File::open(&path)?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            let json = json::parse(&content)?;
            if json["stats"].is_object() {
                // version in 1.18 in vanilla
                for (cate, stats) in json["stats"].entries() {
                    if args.exact {
                        let value = &stats[&args.key];
                        if value.is_null() {
                            continue;
                        }
                        let e = rank.entry(args.key.clone());
                        let vec = &mut e.or_insert((Vec::new(), value.is_number())).0;
                        vec.push((uuid.to_string(), value.clone()));
                        break;
                    } else {
                        for (k, value) in stats.entries() {
                            if k.to_lowercase().contains(&args.key.to_lowercase()) {
                                let e = rank.entry(format!("{}.{}", cate, k));
                                let vec = &mut e.or_insert((Vec::new(), value.is_number())).0;
                                vec.push((uuid.to_string(), value.clone()));
                            }
                        }
                    }
                }
            } else {
                // version in 1.12 with forge
                if args.exact {
                    let value = &json[&args.key];
                    if value.is_null() {
                        continue;
                    }
                    let e = rank.entry(args.key.clone());
                    let vec = &mut e.or_insert((Vec::new(), value.is_number())).0;
                    vec.push((uuid.to_string(), value.clone()));
                } else {
                    for (k, value) in json.entries() {
                        if k.to_lowercase().contains(&args.key.to_lowercase()) {
                            let e = rank.entry(k.to_string());
                            let vec = &mut e.or_insert((Vec::new(), value.is_number())).0;
                            vec.push((uuid.to_string(), value.clone()));
                        }
                    }
                }
            }
        }
    }
    if rank.is_empty() {
        println!("Got empty ranked.");
    }
    for (key, (mut rank, num)) in rank {
        if num {
            rank.sort_unstable_by(|a, b| if args.inverse {
                b.1.as_f64().unwrap().partial_cmp(&a.1.as_f64().unwrap()).unwrap()
            } else {
                a.1.as_f64().unwrap().partial_cmp(&b.1.as_f64().unwrap()).unwrap()
            });
        }
        println!("In stats {}:", key);
        for (idx, (uuid, stats)) in rank.iter().enumerate().take(args.limit) {
            let prefix = if let Some(name) = id_map.map.get(uuid) {
                if args.show_uuid {
                    format!("{}({})", name, uuid)
                } else {
                    format!("{}", name)
                }
            } else {
                uuid.to_string()
            };
            println!("({}) {}: {}", idx + 1, prefix, stats);
        }
        println!();
    }

    Ok(())
}