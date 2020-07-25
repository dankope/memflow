use log::{debug, info};

use memflow_core::connector::{ConnectorArgs, MappedPhysicalMemory};
use memflow_core::mem::{MemoryMap, PhysicalMemory};
use memflow_core::{Error, Result};
use memflow_derive::connector;
use memflow_kvm_ioctl::VMHandle;

// TODO: properly parse args
/// Creates a new KVM Connector instance.
#[connector(name = "kvm")]
pub fn create_connector(args: &ConnectorArgs) -> Result<impl PhysicalMemory> {
    let pid = args
        .get_default()
        .ok_or_else(|| Error::Connector("no pid specified"))?
        .parse::<i32>()
        .ok();
    let vm = VMHandle::try_open(pid).map_err(|_| Error::Connector("Failed to get VM handle"))?;
    let (pid, memslots) = vm
        .info(64)
        .map_err(|_| Error::Connector("Failed to get VM info"))?;
    debug!("pid={} memslots.len()={}", pid, memslots.len());
    for slot in memslots.iter() {
        debug!(
            "{:x}-{:x} -> {:x}-{:x}",
            slot.base,
            slot.base + slot.map_size,
            slot.host_base,
            slot.host_base + slot.map_size
        );
    }
    let mapped_memslots = vm
        .map_vm(64)
        .map_err(|_| Error::Connector("Failed to map VM mem"))?;

    let mut mem_map = MemoryMap::new();

    info!("mmapped {} slots", mapped_memslots.len());
    for slot in mapped_memslots.iter() {
        debug!(
            "{:x}-{:x} -> {:x}-{:x}",
            slot.base,
            slot.base + slot.map_size,
            slot.host_base,
            slot.host_base + slot.map_size
        );
        mem_map.push_remap(
            slot.base.into(),
            slot.map_size as usize,
            slot.host_base.into(),
        );
    }

    Ok(unsafe { MappedPhysicalMemory::from_addrmap_mut(mem_map) })
}
