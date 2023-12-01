/// The price poller should call update_price periodically. `get_price` should
/// Return the price from local cache instead of fetch from internet.
///
/// The key is the asset sygmbol, and the value is the price. Each asset only have one price
/// related. That means we treat xcPHA and PHA as PHA, because they are the same asset essentially
use alloc::vec::Vec;

// TODO: Get price from local cache
pub fn get_price(chain: &str, asset: &Vec<u8>) -> Option<u32> {
    // ETH
    if chain == "Ethereum" && asset == &[0; 20] {
        // 2000 USD
        Some(20000000)
    } else {
        // TODO
        // 0.1 USD
        Some(1000)
    }
}

// TODO: Fetch asset price from internet and save to local cache
#[allow(dead_code, unused_variables)]
pub fn update_price(assets: Vec<&str>) {}
