# Mobile Linux Shell (WASM)

یک شل لینوکسی سبک و کامل که داخل مرورگر اندروید اجرا می‌شود (سبک Termux).

## ویژگی‌ها
- فایل‌سیستم مجازی با ذخیره‌سازی دائمی در IndexedDB
- دستورات پایه: `ls`, `cd`, `pwd`, `cat`, `mkdir`, `rm`, `cp`, `mv`, `echo`, `clear`, `env`, `export`, `grep`
- شبکه: `curl`, `wget`, `ping` (از طریق fetch مرورگر)
- رابط کاربری Termux-style با Extra Keys
- پشتیبانی از آپلود/دانلود فایل

## ساخت

```bash
# پیش‌نیاز
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# بیلد
chmod +x build.sh
./build.sh
