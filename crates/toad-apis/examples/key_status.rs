//! Which API keys are saved (never their values): `cargo run -p toad-apis --example key_status`.
fn main() {
    let s = toad_apis::Secrets::load();
    for k in toad_apis::keys::ALL {
        println!("{k:<22} {}", if s.get(k).is_some() { "set" } else { "-" });
    }
    let n = toad_apis::notify::load_settings();
    println!("smtp server            {}", if n.smtp_host.is_empty() { "-" } else { "set" });
}
