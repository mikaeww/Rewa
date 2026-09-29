//! Small native smoke-test driver. Coordinates are client pixels at the current DPI.
//! Usage: ui-input click <x> <y> | key <virtual-key> | close
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, PostMessageW, WM_CLOSE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
        WM_MOUSEMOVE,
    };
    use windows::core::{PCWSTR, w};
    let args: Vec<_> = std::env::args().collect();
    let window = unsafe { FindWindowW(w!("RewaApplicationWindow"), PCWSTR::null()) }?;
    match args.get(1).map(String::as_str) {
        Some("click") => {
            let x: u16 = args.get(2).ok_or("missing x")?.parse()?;
            let y: u16 = args.get(3).ok_or("missing y")?.parse()?;
            let point = LPARAM(((y as u32) << 16 | x as u32) as isize);
            unsafe {
                PostMessageW(Some(window), WM_MOUSEMOVE, WPARAM(0), point)?;
                PostMessageW(Some(window), WM_LBUTTONDOWN, WPARAM(1), point)?;
                PostMessageW(Some(window), WM_LBUTTONUP, WPARAM(0), point)?;
            }
        }
        Some("key") => {
            let key = WPARAM(args.get(2).ok_or("missing virtual key")?.parse()?);
            unsafe {
                PostMessageW(Some(window), WM_KEYDOWN, key, LPARAM(0))?;
                PostMessageW(Some(window), WM_KEYUP, key, LPARAM(0))?;
            }
        }
        Some("close") => unsafe {
            PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))?;
        },
        _ => return Err("usage: ui-input click x y | key virtual-key | close".into()),
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This driver requires a running Windows Rewa UI.");
}
