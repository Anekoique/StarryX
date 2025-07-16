#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/ioctl.h>
#include <stdint.h>

// 帧缓冲信息结构
typedef struct {
    uint32_t width;
    uint32_t height;
    uint32_t bpp;      // bits per pixel
    uint32_t stride;   // bytes per line
} fb_info_t;

// 尝试通过设备文件访问帧缓冲
int try_framebuffer_access() {
    printf("=== StarryX Graphics Test (C Version) ===\n");
    printf("尝试访问帧缓冲设备...\n");

    // 尝试常见的帧缓冲设备路径
    const char* fb_devices[] = {"/dev/fb0", "/dev/fb", "/dev/framebuffer", NULL};
    int fb_fd = -1;
    
    for (int i = 0; fb_devices[i] != NULL; i++) {
        printf("尝试打开 %s...\n", fb_devices[i]);
        fb_fd = open(fb_devices[i], O_RDWR);
        if (fb_fd >= 0) {
            printf("成功打开帧缓冲设备: %s\n", fb_devices[i]);
            break;
        }
    }
    
    if (fb_fd < 0) {
        printf("无法打开帧缓冲设备\n");
        return -1;
    }
    
    // 假设的帧缓冲参数（通常需要通过 ioctl 获取）
    fb_info_t fb_info = {1024, 768, 32, 1024 * 4};
    
    printf("帧缓冲信息:\n");
    printf("  分辨率: %dx%d\n", fb_info.width, fb_info.height);
    printf("  色深: %d 位\n", fb_info.bpp);
    printf("  步长: %d 字节\n", fb_info.stride);
    
    // 尝试内存映射
    size_t fb_size = fb_info.height * fb_info.stride;
    void* fb_mem = mmap(NULL, fb_size, PROT_READ | PROT_WRITE, MAP_SHARED, fb_fd, 0);
    
    if (fb_mem == MAP_FAILED) {
        printf("内存映射失败\n");
        close(fb_fd);
        return -1;
    }
    
    printf("成功映射帧缓冲内存\n");
    
    // 简单的绘图测试
    uint32_t* pixels = (uint32_t*)fb_mem;
    uint32_t colors[] = {0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF};
    
    printf("开始绘制测试图案...\n");
    
    // 清屏 (黑色)
    memset(fb_mem, 0, fb_size);
    
    // 绘制彩色矩形
    for (int i = 0; i < 6; i++) {
        uint32_t color = colors[i];
        int x = 50 + (i % 3) * 150;
        int y = 50 + (i / 3) * 150;
        int w = 100, h = 100;
        
        for (int dy = 0; dy < h; dy++) {
            for (int dx = 0; dx < w; dx++) {
                int px = x + dx;
                int py = y + dy;
                if (px < fb_info.width && py < fb_info.height) {
                    pixels[py * (fb_info.stride / 4) + px] = color;
                }
            }
        }
        printf("绘制矩形 %d: 位置(%d,%d), 颜色=0x%06X\n", i+1, x, y, color);
    }
    
    // 绘制边框线条
    uint32_t white = 0xFFFFFF;
    // 上边框
    for (int x = 10; x < fb_info.width - 10; x++) {
        pixels[10 * (fb_info.stride / 4) + x] = white;
    }
    // 下边框
    for (int x = 10; x < fb_info.width - 10; x++) {
        pixels[(fb_info.height - 10) * (fb_info.stride / 4) + x] = white;
    }
    // 左边框
    for (int y = 10; y < fb_info.height - 10; y++) {
        pixels[y * (fb_info.stride / 4) + 10] = white;
    }
    // 右边框
    for (int y = 10; y < fb_info.height - 10; y++) {
        pixels[y * (fb_info.stride / 4) + (fb_info.width - 10)] = white;
    }
    
    printf("绘制完成！\n");
    
    // 清理
    munmap(fb_mem, fb_size);
    close(fb_fd);
    
    return 0;
}

// ASCII 艺术模拟图形
void ascii_graphics_demo() {
    printf("\n=== ASCII 图形演示 ===\n");
    printf("模拟彩色矩形:\n");
    
    printf("┌─────────────────────────────────────┐\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│  红色     绿色     蓝色            │\n");
    printf("│                                     │\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│ ██████   ██████   ██████           │\n");
    printf("│  黄色     洋红     青色            │\n");
    printf("└─────────────────────────────────────┘\n");
    
    printf("\n图形测试功能:\n");
    printf("✓ 清屏操作\n");
    printf("✓ 绘制彩色矩形\n");
    printf("✓ 绘制边框线条\n");
    printf("✓ 基本几何图形\n");
}

int main() {
    printf("StarryX 图形系统测试程序 (C 语言版本)\n");
    printf("=====================================\n");
    
    // 首先尝试真实的帧缓冲访问
    int result = try_framebuffer_access();
    
    if (result != 0) {
        printf("\n帧缓冲访问失败，切换到 ASCII 演示模式\n");
        ascii_graphics_demo();
    }
    
    printf("\n图形测试完成。\n");
    printf("如果您看到了 QEMU 图形窗口中的彩色图案，说明硬件图形功能正常！\n");
    
    // 保持程序运行一段时间，让用户能看到结果
    printf("程序将在 5 秒后退出...\n");
    for (int i = 5; i > 0; i--) {
        printf("倒计时: %d\n", i);
        sleep(1);
    }
    
    return 0;
} 