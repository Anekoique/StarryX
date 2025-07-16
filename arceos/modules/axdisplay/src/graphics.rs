//! embedded-graphics集成模块
//! 
//! 为ArceOS提供与embedded-graphics库的集成，支持高级图形绘制功能。

#![cfg(feature = "embedded-graphics")]

use crate::{framebuffer_info, framebuffer_flush};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    text::Text,
};
use micromath::F32Ext;

/// ArceOS帧缓冲的embedded-graphics适配器
/// 
/// 这个结构体实现了embedded-graphics的DrawTarget trait，
/// 使得可以使用embedded-graphics库在ArceOS的帧缓冲上绘制图形。
pub struct AxFrameBuffer {
    fb_base: usize,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
}

impl AxFrameBuffer {
    /// 创建新的AxFrameBuffer实例
    /// 
    /// 自动获取当前系统的帧缓冲信息并初始化适配器。
    pub fn new() -> Self {
        let fb_info = framebuffer_info();
        log::info!("初始化AxFrameBuffer:");
        log::info!("  宽度: {} 像素", fb_info.width);
        log::info!("  高度: {} 像素", fb_info.height);
        log::info!("  帧缓冲地址: 0x{:x}", fb_info.fb_base_vaddr);
        
        Self {
            fb_base: fb_info.fb_base_vaddr,
            width: fb_info.width,
            height: fb_info.height,
            bytes_per_pixel: 4, // 假设32位色深
        }
    }
    
    /// 直接设置像素颜色
    /// 
    /// # 参数
    /// - `x`: X坐标
    /// - `y`: Y坐标  
    /// - `color`: RGB565颜色值
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgb565) {
        if x >= self.width || y >= self.height {
            return;
        }
        
        // 转换Rgb565到32位ARGB格式
        let rgb888 = color.into_storage();
        let r = (((rgb888 >> 11) & 0x1F) << 3) as u32;
        let g = (((rgb888 >> 5) & 0x3F) << 2) as u32;
        let b = ((rgb888 & 0x1F) << 3) as u32;
        let pixel = 0xFF000000u32 | (r << 16) | (g << 8) | b;
        
        let offset = (y * self.width + x) * self.bytes_per_pixel;
        let addr = self.fb_base + offset as usize;
        
        unsafe {
            *(addr as *mut u32) = pixel;
        }
    }
    
    /// 获取帧缓冲的宽度
    pub fn width(&self) -> u32 {
        self.width
    }
    
    /// 获取帧缓冲的高度
    pub fn height(&self) -> u32 {
        self.height
    }
    
    /// 刷新帧缓冲到屏幕
    pub fn flush(&self) {
        framebuffer_flush();
    }
}

impl DrawTarget for AxFrameBuffer {
    type Color = Rgb565;
    type Error = core::convert::Infallible;
    
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels.into_iter() {
            let Point { x, y } = point;
            if x >= 0 && y >= 0 {
                self.set_pixel(x as u32, y as u32, color);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for AxFrameBuffer {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

/// 高级图形绘制器
/// 
/// 提供便捷的绘制方法，封装了常用的图形绘制操作。
pub struct GraphicsRenderer {
    display: AxFrameBuffer,
}

impl GraphicsRenderer {
    /// 创建新的图形绘制器
    pub fn new() -> Self {
        Self {
            display: AxFrameBuffer::new(),
        }
    }
    
    /// 获取内部的AxFrameBuffer引用
    pub fn display_mut(&mut self) -> &mut AxFrameBuffer {
        &mut self.display
    }
    
    /// 清空屏幕
    pub fn clear(&mut self, color: Rgb565) -> Result<(), core::convert::Infallible> {
        self.display.clear(color)
    }
    
    /// 绘制空心矩形
    pub fn draw_rectangle(&mut self, top_left: Point, size: Size, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Rectangle::new(top_left, size)
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(&mut self.display)
    }
    
    /// 绘制实心矩形
    pub fn draw_filled_rectangle(&mut self, top_left: Point, size: Size, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Rectangle::new(top_left, size)
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut self.display)
    }
    
    /// 绘制矩形（可选填充）
    pub fn draw_rectangle_ex(&mut self, top_left: Point, size: Size, color: Rgb565, filled: bool) -> Result<(), core::convert::Infallible> {
        let rect = Rectangle::new(top_left, size);
        if filled {
            rect.into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut self.display)
        } else {
            rect.into_styled(PrimitiveStyle::with_stroke(color, 1))
                .draw(&mut self.display)
        }
    }
    
    /// 绘制空心圆形
    pub fn draw_circle(&mut self, center: Point, diameter: u32, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Circle::new(center, diameter)
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(&mut self.display)
    }
    
    /// 绘制实心圆形
    pub fn draw_filled_circle(&mut self, center: Point, diameter: u32, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Circle::new(center, diameter)
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut self.display)
    }
    
    /// 绘制圆形（可选填充）
    pub fn draw_circle_ex(&mut self, center: Point, diameter: u32, color: Rgb565, filled: bool) -> Result<(), core::convert::Infallible> {
        let circle = Circle::new(center, diameter);
        if filled {
            circle.into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut self.display)
        } else {
            circle.into_styled(PrimitiveStyle::with_stroke(color, 1))
                .draw(&mut self.display)
        }
    }
    
    /// 绘制线条
    pub fn draw_line(&mut self, start: Point, end: Point, color: Rgb565, width: u32) -> Result<(), core::convert::Infallible> {
        Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(color, width))
            .draw(&mut self.display)
    }
    
    /// 绘制空心三角形
    pub fn draw_triangle(&mut self, p1: Point, p2: Point, p3: Point, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(&mut self.display)
    }
    
    /// 绘制实心三角形
    pub fn draw_filled_triangle(&mut self, p1: Point, p2: Point, p3: Point, color: Rgb565) -> Result<(), core::convert::Infallible> {
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut self.display)
    }
    
    /// 绘制三角形（可选填充）
    pub fn draw_triangle_ex(&mut self, p1: Point, p2: Point, p3: Point, color: Rgb565, filled: bool) -> Result<(), core::convert::Infallible> {
        let triangle = Triangle::new(p1, p2, p3);
        if filled {
            triangle.into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut self.display)
        } else {
            triangle.into_styled(PrimitiveStyle::with_stroke(color, 1))
                .draw(&mut self.display)
        }
    }
    
    /// 绘制文本
    pub fn draw_text(&mut self, text: &str, position: Point, color: Rgb565) -> Result<(), core::convert::Infallible> {
        let text_style = MonoTextStyle::new(&FONT_10X20, color);
        Text::new(text, position, text_style)
            .draw(&mut self.display)
            .map(|_| ())
    }
    
    /// 绘制彩色矩形演示
    pub fn draw_colorful_rectangles(&mut self) -> Result<(), core::convert::Infallible> {
        log::info!("绘制彩色矩形演示...");
        
        let colors = [
            Rgb565::RED,
            Rgb565::GREEN, 
            Rgb565::BLUE,
            Rgb565::YELLOW,
            Rgb565::MAGENTA,
            Rgb565::CYAN,
        ];
        
        for (i, &color) in colors.iter().enumerate() {
            let x = 50 + (i as i32 % 3) * 80;
            let y = 50 + (i as i32 / 3) * 60;
            
            self.draw_filled_rectangle(Point::new(x, y), Size::new(60, 40), color)?;
        }
        
        Ok(())
    }
    
    /// 绘制几何图形演示
    pub fn draw_geometry_demo(&mut self) -> Result<(), core::convert::Infallible> {
        log::info!("绘制几何图形演示...");
        
        // 绘制圆形
        self.draw_circle(Point::new(400, 80), 50, Rgb565::WHITE)?;
        self.draw_filled_circle(Point::new(420, 100), 30, Rgb565::RED)?;
        
        // 绘制三角形
        self.draw_triangle(
            Point::new(100, 200),
            Point::new(150, 280),
            Point::new(50, 280),
            Rgb565::GREEN
        )?;
        
        // 绘制线条
        self.draw_line(Point::new(200, 200), Point::new(300, 280), Rgb565::CYAN, 4)?;
        
        Ok(())
    }
    
    /// 绘制网格
    pub fn draw_grid(&mut self, start_x: i32, start_y: i32, width: i32, height: i32, cell_size: i32, color: Rgb565) -> Result<(), core::convert::Infallible> {
        // 绘制垂直线
        for i in 0..=(width / cell_size) {
            let x = start_x + i * cell_size;
            self.draw_line(
                Point::new(x, start_y), 
                Point::new(x, start_y + height), 
                color, 
                1
            )?;
        }
        
        // 绘制水平线
        for i in 0..=(height / cell_size) {
            let y = start_y + i * cell_size;
            self.draw_line(
                Point::new(start_x, y), 
                Point::new(start_x + width, y), 
                color, 
                1
            )?;
        }
        
        Ok(())
    }
    
    /// 创建动画圆点（椭圆运动）
    pub fn animate_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, steps: u32, delay_loops: u32) -> Result<(), core::convert::Infallible> {
        log::info!("开始椭圆动画演示...");
        
        for frame in 0..steps {
            let t = frame as f32 * 0.1;
            let x = center.x + (t.sin() * radius_x) as i32;
            let y = center.y + (t.cos() * radius_y) as i32;
            
            // 绘制移动的圆点
            self.draw_filled_circle(Point::new(x, y), 8, Rgb565::RED)?;
            self.display.flush();
            
            // 简单延时
            for _ in 0..delay_loops {
                core::hint::spin_loop();
            }
            
            // 清除之前的圆点
            self.draw_filled_circle(Point::new(x, y), 8, Rgb565::BLACK)?;
        }
        
        log::info!("动画演示完成");
        Ok(())
    }
    
    /// 刷新显示
    pub fn flush(&self) {
        self.display.flush();
    }
} 