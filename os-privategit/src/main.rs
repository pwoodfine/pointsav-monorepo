use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PRODUCT: &str = "os-privategit";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{PRODUCT} {VERSION}");
        return;
    }

    println!("PointSav PrivateGit OS Image");
    println!("  product: {PRODUCT}");
    println!("  version: {VERSION}");

    match std::env::var("POINTSAV_LICENSE") {
        Ok(key) if !key.is_empty() => {
            println!("  license: present");
            println!("  status:  ok");
        }
        _ => {
            eprintln!("  license: absent");
            eprintln!("  status:  unlicensed");
            eprintln!();
            eprintln!("Set POINTSAV_LICENSE=<token> to activate.");
            eprintln!("Purchase at https://software.pointsav.com");
            process::exit(2);
        }
    }
}
