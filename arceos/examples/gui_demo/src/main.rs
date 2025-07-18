#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;

#[cfg(all(feature = "axstd", feature = "display"))]
use axdisplay::prelude::*;

#[cfg(all(feature = "axstd", feature = "display"))]
use axdisplay::framebuffer_info;

/// 文件夹图标
#[cfg(all(feature = "axstd", feature = "display"))]
struct FolderIcon {
    x: i32,
    y: i32,
    name: &'static str,
}

#[cfg(all(feature = "axstd", feature = "display"))]
impl FolderIcon {
    fn new(x: i32, y: i32, name: &'static str) -> Self {
        Self { x, y, name }
    }
    
    fn draw(&self, renderer: &mut GraphicsRenderer) -> Result<(), core::convert::Infallible> {
        let icon_width = 48;
        let icon_height = 40;
        
        // 绘制文件夹底部
        renderer.draw_filled_rectangle(
            Point::new(self.x + 8, self.y + 8),
            Size::new(icon_width - 8, icon_height - 8),
            Rgb565::new(31, 25, 0) // 黄色文件夹
        )?;
        
        // 绘制文件夹标签页
        renderer.draw_filled_rectangle(
            Point::new(self.x, self.y),
            Size::new(20, 12),
            Rgb565::new(31, 25, 0)
        )?;
        
        // 绘制文件夹边框
        renderer.draw_rectangle(
            Point::new(self.x + 8, self.y + 8),
            Size::new(icon_width - 8, icon_height - 8),
            Rgb565::new(20, 16, 0)
        )?;
        
        // 绘制标签页边框
        renderer.draw_rectangle(
            Point::new(self.x, self.y),
            Size::new(20, 12),
            Rgb565::new(20, 16, 0)
        )?;
        
        // 绘制文件夹名称
        let text_x = self.x + (icon_width as i32 - (self.name.len() as i32 * 6)) / 2;
        let text_y = self.y + icon_height as i32 + 5;
        renderer.draw_text(self.name, Point::new(text_x, text_y), Rgb565::WHITE)?;
        
        Ok(())
    }
}

/// 简单桌面
#[cfg(all(feature = "axstd", feature = "display"))]
struct SimpleDesktop {
    folders: [FolderIcon; 6],
    screen_width: u32,
    screen_height: u32,
}

#[cfg(all(feature = "axstd", feature = "display"))]
impl SimpleDesktop {
    fn new(screen_width: u32, screen_height: u32) -> Self {
        let margin = 50;
        let folder_spacing_x = 100;
        let folder_spacing_y = 80;
        
        Self {
            folders: [
                FolderIcon::new(margin, margin, "My Documents"),
                FolderIcon::new(margin + folder_spacing_x, margin, "Programs"),
                FolderIcon::new(margin + folder_spacing_x * 2, margin, "System"),
                FolderIcon::new(margin, margin + folder_spacing_y, "Games"),
                FolderIcon::new(margin + folder_spacing_x, margin + folder_spacing_y, "Tools"),
                FolderIcon::new(margin + folder_spacing_x * 2, margin + folder_spacing_y, "Recycle Bin"),
            ],
            screen_width,
            screen_height,
        }
    }
    
    fn draw(&self, renderer: &mut GraphicsRenderer) -> Result<(), core::convert::Infallible> {
        // 绘制桌面背景 - 经典的青绿色桌面
        renderer.clear(Rgb565::new(0, 20, 20))?;
        
        // 绘制所有文件夹图标
        for folder in &self.folders {
            folder.draw(renderer)?;
        }
        
        // 绘制任务栏
        let taskbar_height = 28;
        let taskbar_y = (self.screen_height - taskbar_height) as i32;
        renderer.draw_filled_rectangle(
            Point::new(0, taskbar_y),
            Size::new(self.screen_width, taskbar_height),
            Rgb565::new(12, 12, 12) // 深灰色任务栏
        )?;
        
        // 绘制任务栏边框
        renderer.draw_rectangle(
            Point::new(0, taskbar_y),
            Size::new(self.screen_width, taskbar_height),
            Rgb565::new(20, 20, 20)
        )?;
        
        // 绘制开始按钮
        renderer.draw_filled_rectangle(
            Point::new(2, taskbar_y + 2),
            Size::new(50, 24),
            Rgb565::new(16, 20, 16)
        )?;
        renderer.draw_rectangle(
            Point::new(2, taskbar_y + 2),
            Size::new(50, 24),
            Rgb565::WHITE
        )?;
        renderer.draw_text("Start", Point::new(12, taskbar_y + 8), Rgb565::WHITE)?;
        
        // 绘制时间显示
        let time_text = "12:34 PM";
        let time_x = (self.screen_width as i32 - (time_text.len() as i32 * 6) - 10).max(60);
        renderer.draw_text(
            time_text,
            Point::new(time_x, taskbar_y + 8),
            Rgb565::WHITE
        )?;
        
        // 绘制桌面标题
        renderer.draw_text(
            "ArceOS Desktop v1.0",
            Point::new(10, 10),
            Rgb565::WHITE
        )?;
        
        Ok(())
    }
}

/// 简单桌面演示函数
#[cfg(all(feature = "axstd", feature = "display"))]
fn simple_desktop_demo() -> Result<(), core::convert::Infallible> {
    println!("启动ArceOS简单桌面演示...");
    
    // 获取实际屏幕分辨率
    let fb_info = framebuffer_info();
    println!("检测到屏幕分辨率: {}x{}", fb_info.width, fb_info.height);
    
    let mut renderer = GraphicsRenderer::new();
    let desktop = SimpleDesktop::new(fb_info.width, fb_info.height);
    
    // 绘制桌面
    desktop.draw(&mut renderer)?;
    renderer.flush();
    println!("桌面渲染完成");
    
    Ok(())
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("ArceOS 简单桌面演示");
    
    #[cfg(all(feature = "axstd", feature = "display"))]
    {
        if let Err(_) = simple_desktop_demo() {
            println!("桌面演示失败");
            return;
        }
        
        println!("桌面演示完成！");
        println!("展示的功能包括:");
        println!("- 经典桌面背景");
        println!("- 文件夹图标");
        println!("- 任务栏");
        println!("- 系统时间显示");
    }
    
    #[cfg(not(all(feature = "axstd", feature = "display")))]
    {
        println!("显示功能未启用。请使用 --features display 编译");
    }
    
    println!("程序执行完成，进入循环...");
    loop {
        #[cfg(feature = "axstd")]
        axstd::thread::sleep(axstd::time::Duration::from_secs(1));
    }
} 