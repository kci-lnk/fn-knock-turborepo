use std::net::{Ipv4Addr, Ipv6Addr};

pub(crate) fn ipv4_prefix_len(mask: Ipv4Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

pub(crate) fn ipv6_prefix_len(mask: Ipv6Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_ipv4_and_ipv6_mask_bits_like_existing_callers() {
        assert_eq!(ipv4_prefix_len(Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(
            ipv6_prefix_len("ffff:ffff:ffff:ffff::".parse::<Ipv6Addr>().unwrap()),
            64
        );
    }
}
