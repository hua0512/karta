#[kovi::plugin]
async fn main() {
    karta_kovi::register_plugin().await;
}
