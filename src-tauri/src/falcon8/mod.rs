use std::time::Duration;

use rusb::{Context, Device, DeviceHandle, Direction, Recipient, RequestType, Result, UsbContext};

mod consts;

pub use consts::*;
pub mod protocol;

#[derive(Debug)]
pub struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

#[derive(Debug)]
pub struct Falcon8<'a, T: UsbContext> {
    pub context: &'a mut T,
    pub device: Device<T>,
    pub handle: DeviceHandle<T>,
}

impl<'a> Falcon8<'a, Context> {
    pub fn new() -> Result<Vec<Self>> {
        let mut context = Context::new()?;
        let devices = Self::open_devices(&mut context, VID, PID)?;

        if devices.is_empty() {
            return Err(rusb::Error::NotFound);
        }

        Ok(devices)
    }
}

impl<'a, T: UsbContext> Falcon8<'a, T> {
    fn open_devices(context: &mut T, vid: u16, pid: u16) -> Result<Vec<Falcon8<T>>> {
        let devices = context.devices()?;
        let mut result = Vec::new();

        for device in devices.iter() {
            let Ok(device_desc) = device.device_descriptor() else {
                continue;
            };

            if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
                if let Ok(handle) = device.open() {
                    result.push(Falcon8 {
                        context,
                        device,
                        handle,
                    });
                }
            }
        }

        Ok(result)
    }

    pub fn print_device_info(&self) -> Result<()> {
        let device_desc = self.handle.device().device_descriptor()?;
        let timeout = std::time::Duration::from_secs(1);
        let languages = self.handle.read_languages(timeout)?;

        println!(
            "Active configuration: {}",
            self.handle.active_configuration()?
        );

        if !languages.is_empty() {
            let language = languages[0];
            println!("Language: {:?}", language);

            println!(
                "Manufacturer: {}",
                self.handle
                    .read_manufacturer_string(language, &device_desc, timeout)
                    .unwrap_or_else(|_| "Not Found".to_string())
            );
            println!(
                "Product: {}",
                self.handle
                    .read_product_string(language, &device_desc, timeout)
                    .unwrap_or_else(|_| "Not Found".to_string())
            );
            println!(
                "Serial Number: {}",
                self.handle
                    .read_serial_number_string(language, &device_desc, timeout)
                    .unwrap_or_else(|_| "Not Found".to_string())
            );
        }
        Ok(())
    }

    pub fn find_readable_endpoints(&self) -> Result<Vec<Endpoint>> {
        let config_desc = match self.device.config_descriptor(0) {
            Ok(c) => c,
            Err(_) => return Err(rusb::Error::NoDevice),
        };
        let mut endpoints = vec![];

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    println!("{:#?}", endpoint_desc);
                    endpoints.push(Endpoint {
                        config: config_desc.number(),
                        iface: interface_desc.interface_number(),
                        setting: interface_desc.setting_number(),
                        address: endpoint_desc.address(),
                    });
                }
            }
        }

        println!("Endpoints: {:?}", endpoints);
        Ok(endpoints)
    }

    pub fn claim_interfaces(&self) -> Result<()> {
        let config_desc = match self.device.config_descriptor(0) {
            Ok(c) => c,
            Err(_) => return Err(rusb::Error::NoDevice),
        };
        println!("got desc");
        for iface in config_desc.interfaces() {
            // claim
            println!("claiming {}", iface.number());
            self.handle.claim_interface(iface.number())?;
            println!("claimed {}", iface.number());
            break;
        }
        Ok(())
    }

    pub fn release_interfaces(&self) -> Result<()> {
        let config_desc = match self.device.config_descriptor(0) {
            Ok(c) => c,
            Err(_) => return Err(rusb::Error::NoDevice),
        };

        for iface in config_desc.interfaces() {
            // release
            self.handle.release_interface(iface.number())?;
        }
        Ok(())
    }

    fn detach_kernel_driver(&self, endpoint: &Endpoint) -> Result<()> {
        let has_kernel_driver = match self.handle.kernel_driver_active(endpoint.iface) {
            Ok(true) => {
                self.handle.detach_kernel_driver(endpoint.iface)?;
                true
            }
            _ => false,
        };
        if has_kernel_driver {
            println!("Detached kernel driver");
        }
        Ok(())
    }

    fn reattach_kernel_driver(&mut self, endpoint: &Endpoint) -> Result<()> {
        let has_kernel_driver = match self.handle.kernel_driver_active(endpoint.iface) {
            Ok(true) => {
                self.handle.detach_kernel_driver(endpoint.iface)?;
                true
            }
            _ => false,
        };
        if has_kernel_driver {
            println!("Reattached kernel driver");
        }
        Ok(())
    }

    pub fn get_report(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        let endpoint = &self.find_readable_endpoints()?[0];

        println!("endpoint!: {:?}", endpoint);
        self.detach_kernel_driver(&endpoint)?;
        println!("detached kernel driver");
        self.claim_interfaces()?;
        println!("claimed ifaces");

        println!("Reading!");
        let size = self.handle.read_control(
            rusb::request_type(Direction::In, RequestType::Class, Recipient::Interface),
            0x01,
            0x0307,
            0x0002,
            data.as_mut_slice(),
            Duration::from_secs(1),
        )?;
        println!("size: {:?}", size);

        self.release_interfaces()?;
        println!("released ifaces");
        self.reattach_kernel_driver(endpoint)?;
        println!("reattached kernel driver");
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_report() {
        let mut falcons = Falcon8::new().unwrap();
        for falcon in falcons {
            falcon.print_device_info().unwrap();
            falcon.get_report().unwrap();
        }
    }
}
