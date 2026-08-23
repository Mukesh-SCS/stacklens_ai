/// Normalize a PCI bus ID to `dddddddd:bb:dd.f` (lowercase hex).
pub fn normalize_pci_bus_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches("PCI:").trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    let (domain, bus, devfn) = match parts.as_slice() {
        [domain, bus, devfn] => (*domain, *bus, *devfn),
        [bus, devfn] => ("0", *bus, *devfn),
        _ => return None,
    };

    let (device, function) = devfn.split_once('.')?;
    if !is_hex(domain) || !is_hex(bus) || !is_hex(device) || !is_hex(function) {
        return None;
    }

    Some(format!(
        "{:08x}:{:02x}:{:02x}.{:x}",
        parse_hex(domain)?,
        parse_hex(bus)?,
        parse_hex(device)?,
        parse_hex(function)?
    ))
}

fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn parse_hex(value: &str) -> Option<u32> {
    u32::from_str_radix(value, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::normalize_pci_bus_id;

    #[test]
    fn sysfs_and_smi_forms_match() {
        assert_eq!(
            normalize_pci_bus_id("0000:01:00.0").as_deref(),
            Some("00000000:01:00.0")
        );
        assert_eq!(
            normalize_pci_bus_id("00000000:01:00.0").as_deref(),
            Some("00000000:01:00.0")
        );
        assert_eq!(
            normalize_pci_bus_id("01:00.0").as_deref(),
            Some("00000000:01:00.0")
        );
        assert_eq!(
            normalize_pci_bus_id("0000:21:00.0").as_deref(),
            Some("00000000:21:00.0")
        );
    }

    #[test]
    fn case_and_prefix() {
        assert_eq!(
            normalize_pci_bus_id("PCI:0000:0A:00.0").as_deref(),
            Some("00000000:0a:00.0")
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(normalize_pci_bus_id(""), None);
        assert_eq!(normalize_pci_bus_id("not-pci"), None);
        assert_eq!(normalize_pci_bus_id("0000:01:00"), None);
    }
}
