use axdisplay::{framebuffer_info, framebuffer_flush};


// 简单的像素画图函数
fn draw_pixel(fb_base: usize, x: u32, y: u32, color: u32, width: u32, bpp: u32) {
    // 边界检查
    if fb_base == 0 {
        return;
    }
    
    let bytes_per_pixel = bpp / 8;
    let offset = (y * width + x) * bytes_per_pixel;
    let addr = fb_base + offset as usize;
    
    // 基本的地址有效性检查
    if addr < fb_base {
        ax_println!("警告：计算的地址 0x{:x} 小于基地址 0x{:x}", addr, fb_base);
        return;
    }
    
    unsafe {
        match bytes_per_pixel {
            4 => core::ptr::write_volatile(addr as *mut u32, color),
            3 => {
                core::ptr::write_volatile(addr as *mut u8, (color & 0xFF) as u8);
                core::ptr::write_volatile((addr + 1) as *mut u8, ((color >> 8) & 0xFF) as u8);
                core::ptr::write_volatile((addr + 2) as *mut u8, ((color >> 16) & 0xFF) as u8);
            }
            2 => core::ptr::write_volatile(addr as *mut u16, color as u16),
            1 => core::ptr::write_volatile(addr as *mut u8, color as u8),
            _ => {
                ax_println!("不支持的每像素字节数: {}", bytes_per_pixel);
            }
        }
    }
}

// 画矩形
fn draw_rect(fb_base: usize, x: u32, y: u32, w: u32, h: u32, color: u32, screen_width: u32, bpp: u32) {
    for dy in 0..h {
        for dx in 0..w {
            if x + dx < screen_width {
                draw_pixel(fb_base, x + dx, y + dy, color, screen_width, bpp);
            }
        }
    }
}

// 画线 (简单版本)
fn draw_line(fb_base: usize, x0: u32, y0: u32, x1: u32, y1: u32, color: u32, screen_width: u32, bpp: u32) {
    let dx = if x1 > x0 { x1 - x0 } else { x0 - x1 };
    let dy = if y1 > y0 { y1 - y0 } else { y0 - y1 };
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx as i32 - dy as i32;
    
    let mut x = x0 as i32;
    let mut y = y0 as i32;
    
    loop {
        draw_pixel(fb_base, x as u32, y as u32, color, screen_width, bpp);
        
        if x == x1 as i32 && y == y1 as i32 {
            break;
        }
        
        let e2 = 2 * err;
        if e2 > -(dy as i32) {
            err -= dy as i32;
            x += sx;
        }
        if e2 < dx as i32 {
            err += dx as i32;
            y += sy;
        }
    }
}

// 帧缓冲画图测试
pub fn test_framebuffer_drawing() {
    ax_println!("开始帧缓冲画图测试...");
    
    // 获取帧缓冲信息
    ax_println!("正在获取帧缓冲信息...");
    let fb_info = framebuffer_info();
    ax_println!("帧缓冲信息获取成功:");
    ax_println!("  宽度: {} 像素", fb_info.width);
    ax_println!("  高度: {} 像素", fb_info.height);
    ax_println!("  帧缓冲大小: {} bytes", fb_info.fb_size);
    
    // 获取帧缓冲基地址
    let fb_base = fb_info.fb_base_vaddr;
    let width = fb_info.width;
    let height = fb_info.height;
    let bpp = 32; // 假设32位色深 (4字节每像素)
    
    ax_println!("  帧缓冲地址: 0x{:x}", fb_base);
    
    // 验证帧缓冲参数
    if fb_base == 0 {
        ax_println!("错误：帧缓冲地址为0！");
        return;
    }
    if width == 0 || height == 0 {
        ax_println!("错误：帧缓冲尺寸无效！width={}, height={}", width, height);
        return;
    }
    
    ax_println!("开始清空屏幕...");
    
    // 清空屏幕 (黑色背景) - 只清理一小部分来测试
    let test_pixels = core::cmp::min(width * height, 1000); // 只测试前1000个像素
    for i in 0..test_pixels {
        let x = i % width;
        let y = i / width;
        draw_pixel(fb_base, x, y, 0x000000, width, bpp);
        
        // 每100个像素打印一次进度
        if i % 100 == 0 {
            ax_println!("已处理 {} 像素", i);
        }
    }
    
    ax_println!("屏幕清空完成");
    
    // 画一些彩色矩形
    let colors = [0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF];
    let rect_size = 50;
    let spacing = 60;
    
    for (i, &color) in colors.iter().enumerate() {
        let x = 50 + (i as u32 % 3) * spacing;
        let y = 50 + (i as u32 / 3) * spacing;
        
        if x + rect_size <= width && y + rect_size <= height {
            draw_rect(fb_base, x, y, rect_size, rect_size, color, width, bpp);
            ax_println!("绘制矩形 {} 在位置 ({}, {}), 颜色: 0x{:06x}", i, x, y, color);
        }

    }
    
    // 画一些线条
    if width > 400 && height > 300 {
        // 画X形交叉线
        draw_line(fb_base, 250, 100, 350, 200, 0xFFFFFF, width, bpp);
        draw_line(fb_base, 350, 100, 250, 200, 0xFFFFFF, width, bpp);
        ax_println!("绘制X形交叉线");
        
        // 画边框
        // 上边框
        draw_line(fb_base, 10, 10, width - 10, 10, 0xFFFFFF, width, bpp);
        // 下边框
        draw_line(fb_base, 10, height - 10, width - 10, height - 10, 0xFFFFFF, width, bpp);
        // 左边框
        draw_line(fb_base, 10, 10, 10, height - 10, 0xFFFFFF, width, bpp);
        // 右边框
        draw_line(fb_base, width - 10, 10, width - 10, height - 10, 0xFFFFFF, width, bpp);
        ax_println!("绘制屏幕边框");
    }
    
    // 刷新帧缓冲
    framebuffer_flush();
    ax_println!("帧缓冲绘制完成，已刷新到屏幕");
}