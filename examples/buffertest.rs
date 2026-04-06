use libbladerf_rs::Error;
use std::time::Duration;

// struct NiosEndpoints {
//     ep_out: Endpoint<Bulk, Out>,
//     ep_in: Endpoint<Bulk, In>,
//     ep_out_buffer: Buffer,
//     ep_in_buffer: Buffer,
// }

fn main() -> Result<(), Error> {
    let _t = Duration::from_secs(1);
    // let endpoints = self.ensure_nios_endpoints()?;
    // // Submit OUT transfer
    // endpoints.ep_out.submit(endpoints.ep_out_buffer);
    // let mut response = endpoints
    //     .ep_out
    //     .wait_next_complete(t)
    //     .ok_or(Error::Timeout)?;
    // // Should we handle resetting on stalled transfers here?
    // response.status?;
    //
    // // For the IN transfer, we use a different also preallocated buffer
    // endpoints.ep_in.submit(endpoints.ep_in_buffer);
    // response = endpoints
    //     .ep_in
    //     .wait_next_complete(t)
    //     .ok_or(Error::Timeout)?;
    // // Should we handle resetting on stalled transfers here?
    // response.status?;
    Ok(())
}
