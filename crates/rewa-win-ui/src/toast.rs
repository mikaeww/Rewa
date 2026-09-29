//! The note that a clip was saved, shown by the tray over whatever runs, game included.
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetMonitorInfoW,
    HBITMAP, HDC, HGDIOBJ, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow, SelectObject,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetForegroundWindow, HWND_TOPMOST, RegisterClassW, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

const SHOWN_FOR: Duration = Duration::from_millis(2_600);
const FADE: Duration = Duration::from_millis(220);
const CARD_WIDTH: f32 = 320.0;
const CARD_HEIGHT: f32 = 68.0;
/// Transparent room around the card for its shadow.
const SHADOW: f32 = 14.0;
/// Distance from the monitor's top right corner.
const MARGIN: f32 = 20.0;

pub struct Toast {
    window: HWND,
    d2d: ID2D1Factory,
    write: IDWriteFactory,
    wic: IWICImagingFactory,
    surface: Option<Surface>,
    shown: Option<Instant>,
}

/// The rendered card in a 32-bit DIB; fading only changes the window's alpha.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

impl Toast {
    pub fn new() -> Result<Self, String> {
        // the tray thread may not have COM yet; WIC needs it for the ghost
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let class = WNDCLASSW {
            lpfnWndProc: Some(passive_proc),
            lpszClassName: w!("RewaToast"),
            ..Default::default()
        };
        unsafe { RegisterClassW(&class) };
        // layered and transparent: never takes focus or a click away from the game
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("RewaToast"),
                w!("Rewa"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        let d2d =
            unsafe { D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
                .map_err(|error| error.to_string())?;
        let write = unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED) }
            .map_err(|error| error.to_string())?;
        let wic = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| error.to_string())?;
        Ok(Self {
            window,
            d2d,
            write,
            wic,
            surface: None,
            shown: None,
        })
    }

    /// Shows the card at the top right of the monitor in use; a second save
    /// before it faded replaces the text and starts the clock again.
    pub fn show(&mut self, title: &str, detail: &str) -> Result<(), String> {
        let monitor = unsafe { MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTOPRIMARY) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return Err("Windows reported no monitor for the clip note".into());
        }
        let (mut dpi_x, mut dpi_y) = (96_u32, 96_u32);
        let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
        let scale = dpi_x.max(96) as f32 / 96.0;
        let width = ((CARD_WIDTH + SHADOW * 2.0) * scale).round() as i32;
        let height = ((CARD_HEIGHT + SHADOW * 2.0) * scale).round() as i32;
        let area = info.rcMonitor;
        let position = POINT {
            x: area.right - width - ((MARGIN - SHADOW) * scale).round() as i32,
            y: area.top + ((MARGIN - SHADOW) * scale).round() as i32,
        };

        self.surface = None;
        let surface = self.render(width, height, scale, title, detail)?;
        let blend = blend(255);
        unsafe {
            UpdateLayeredWindow(
                self.window,
                None,
                Some(&position),
                Some(&SIZE {
                    cx: width,
                    cy: height,
                }),
                Some(surface.dc),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
        .map_err(|error| error.to_string())?;
        self.surface = Some(surface);
        unsafe {
            let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        self.shown = Some(Instant::now());
        Ok(())
    }

    /// Advances the fade; returns false once the card is gone and the timer can stop.
    pub fn tick(&mut self) -> bool {
        let Some(shown) = self.shown else {
            return false;
        };
        let elapsed = shown.elapsed();
        if elapsed < SHOWN_FOR {
            return true;
        }
        let fade = (elapsed - SHOWN_FOR).as_secs_f32() / FADE.as_secs_f32();
        if fade >= 1.0 {
            unsafe {
                let _ = ShowWindow(self.window, SW_HIDE);
            }
            self.surface = None;
            self.shown = None;
            return false;
        }
        let blend = blend(((1.0 - fade) * 255.0).round() as u8);
        let _ = unsafe {
            UpdateLayeredWindow(
                self.window,
                None,
                None,
                None,
                None,
                None,
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        };
        true
    }

    fn render(
        &self,
        width: i32,
        height: i32,
        scale: f32,
        title: &str,
        detail: &str,
    ) -> Result<Surface, String> {
        let dc = unsafe { CreateCompatibleDC(None) };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = match unsafe {
            CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        } {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = unsafe { DeleteDC(dc) };
                return Err(error.to_string());
            }
        };
        let previous = unsafe { SelectObject(dc, bitmap.into()) };
        let surface = Surface {
            dc,
            bitmap,
            previous,
        };
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            // the card is laid out in logical pixels and drawn at the monitor's scale
            dpiX: 96.0 * scale,
            dpiY: 96.0 * scale,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = unsafe { self.d2d.CreateDCRenderTarget(&properties) }
            .map_err(|error| error.to_string())?;
        unsafe {
            target.BindDC(
                surface.dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )
        }
        .map_err(|error| error.to_string())?;
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&D2D1_COLOR_F::default()));
        }
        let drawn = self.draw(&target, title, detail);
        let ended = unsafe { target.EndDraw(None, None) }.map_err(|error| error.to_string());
        drawn.and(ended)?;
        Ok(surface)
    }

    fn draw(&self, target: &ID2D1DCRenderTarget, title: &str, detail: &str) -> Result<(), String> {
        let card = D2D_RECT_F {
            left: SHADOW,
            top: SHADOW,
            right: SHADOW + CARD_WIDTH,
            bottom: SHADOW + CARD_HEIGHT,
        };
        for (spread, alpha) in [(10.0, 0.05), (6.0, 0.08), (3.0, 0.12)] {
            fill(
                target,
                grow(card, spread, 3.0),
                color(0x000000, alpha),
                12.0 + spread,
            )?;
        }
        fill(target, card, color(0x1c1c1c, 0.97), 12.0)?;
        let edge = brush(target, color(0x333333, 1.0))?;
        unsafe {
            target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: grow(card, -0.5, 0.0),
                    radiusX: 12.0,
                    radiusY: 12.0,
                },
                &edge,
                1.0,
                None,
            );
        }
        // the bar the reference carries, in ink: Rewa spends colour only on destructive things
        fill(
            target,
            D2D_RECT_F {
                left: card.left + 10.0,
                top: card.top + 14.0,
                right: card.left + 13.0,
                bottom: card.bottom - 14.0,
            },
            color(0xededed, 1.0),
            1.5,
        )?;
        if let Ok(ghost) = self.ghost(target) {
            let middle = (card.top + card.bottom) / 2.0;
            unsafe {
                target.DrawBitmap(
                    &ghost,
                    Some(&D2D_RECT_F {
                        left: card.left + 24.0,
                        top: middle - 15.0,
                        right: card.left + 54.0,
                        bottom: middle + 15.0,
                    }),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        }
        let text_left = card.left + 68.0;
        self.text(
            target,
            title,
            &self.format(14.0, true)?,
            D2D_RECT_F {
                left: text_left,
                top: card.top + 12.0,
                right: card.right - 16.0,
                bottom: card.top + 34.0,
            },
            0xededed,
        )?;
        self.text(
            target,
            detail,
            &self.format(12.5, false)?,
            D2D_RECT_F {
                left: text_left,
                top: card.top + 34.0,
                right: card.right - 16.0,
                bottom: card.bottom - 12.0,
            },
            0x949494,
        )
    }

    fn format(&self, size: f32, semibold: bool) -> Result<IDWriteTextFormat, String> {
        let format = unsafe {
            self.write.CreateTextFormat(
                w!("Segoe UI Variable Text"),
                None,
                if semibold {
                    DWRITE_FONT_WEIGHT_SEMI_BOLD
                } else {
                    DWRITE_FONT_WEIGHT_NORMAL
                },
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-US"),
            )
        }
        .map_err(|error| error.to_string())?;
        unsafe {
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        }
        Ok(format)
    }

    fn text(
        &self,
        target: &ID2D1DCRenderTarget,
        value: &str,
        format: &IDWriteTextFormat,
        area: D2D_RECT_F,
        ink: u32,
    ) -> Result<(), String> {
        let ink = brush(target, color(ink, 1.0))?;
        let value = value.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &value,
                format,
                &area,
                &ink,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    fn ghost(
        &self,
        target: &ID2D1DCRenderTarget,
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1Bitmap, String> {
        const GHOST_PNG: &[u8] = include_bytes!("ghost-64.png");
        let stream = unsafe { self.wic.CreateStream() }.map_err(|error| error.to_string())?;
        unsafe { stream.InitializeFromMemory(GHOST_PNG) }.map_err(|error| error.to_string())?;
        let decoder = unsafe {
            self.wic.CreateDecoderFromStream(
                &stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )
        }
        .map_err(|error| error.to_string())?;
        let frame = unsafe { decoder.GetFrame(0) }.map_err(|error| error.to_string())?;
        let converter =
            unsafe { self.wic.CreateFormatConverter() }.map_err(|error| error.to_string())?;
        unsafe {
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
        }
        .map_err(|error| error.to_string())?;
        unsafe { target.CreateBitmapFromWicBitmap(&converter, None) }
            .map_err(|error| error.to_string())
    }
}

unsafe extern "system" fn passive_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn blend(alpha: u8) -> BLENDFUNCTION {
    BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: AC_SRC_ALPHA as u8,
    }
}

fn grow(area: D2D_RECT_F, by: f32, down: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: area.left - by,
        top: area.top - by + down,
        right: area.right + by,
        bottom: area.bottom + by + down,
    }
}

fn color(rgb: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: alpha,
    }
}

fn brush(
    target: &ID2D1DCRenderTarget,
    fill: D2D1_COLOR_F,
) -> Result<windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush, String> {
    unsafe { target.CreateSolidColorBrush(&fill, None) }.map_err(|error| error.to_string())
}

fn fill(
    target: &ID2D1DCRenderTarget,
    area: D2D_RECT_F,
    fill: D2D1_COLOR_F,
    radius: f32,
) -> Result<(), String> {
    let fill = brush(target, fill)?;
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: area,
                radiusX: radius,
                radiusY: radius,
            },
            &fill,
        );
    }
    Ok(())
}
