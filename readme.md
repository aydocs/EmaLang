# EMA Programming Language (Evren Mimari)

EMA, modern web teknolojilerini (HTML, CSS, JS, PHP) tek bir ekosistemde birleştiren, yüksek performanslı ve geliştirici dostu bir programlama dilidir.

## 🚀 Hızlı Başlangıç

EMA'yı sisteminize kurduktan sonra (NPM üzerinden `npm install -g emalang`), yeni bir projeye saniyeler içinde başlayabilirsiniz:

```bash
# Yeni bir proje başlat
ema init

# REPL Modunda çalıştır (İnteraktif)
ema
# veya
ema repl

# Projeyi çalıştır
ema ema/project.ema
```

## ✨ Temel Özellikler

- **Birleşik Ekosistem**: HTML, CSS, JS ve PHP mantığını tek bir `.ema` dosyasında kullanın.
- **Modern JavaScript**: `async/await`, `arrow functions` ve ES6+ özelliklerini tam destekler.
- **Akıllı CSS**: Her bileşene özel (Scoped) CSS ile çakışmaları önleyin.
- **Güçlü Backend**: Rust hızında çalışan backend motoru ve PHP tarzı kolay sözdizimi.
- **Dinamik Port**: Port 3000 doluysa otomatik olarak bir üst portu (3001, 3002...) bulur ve oradan başlar.
- **Temiz CLI**: `--verbose` flag'i ile sadece ihtiyacınız olan logları görün.

## 🛠 Standart Kütüphane (StdLib)

EMA, geliştirme sürecini hızlandıran zengin bir standart kütüphane ile gelir:

### `std::http`
- `serve()`: Sunucuyu başlatır (Varsayılan: 3000).
- `route(method, path, handler)`: API rotaları tanımlar.

### `std::db`
- `migrate()`: Veritabanı şemasını oluşturur/günceller.
- `registerModel(name, fields)`: Veri modelleri tanımlar.

### `std::file` (veya `std::fs`)
- `read(path)`: Dosya içeriğini okur.
- `write(path, content)`: Dosyaya yazar.
- `exists(path)`: Dosyanın varlığını kontrol eder.
- `delete(path)`: Dosyayı siler.

### `std::json`
- `parse(string)`: JSON string'ini EMA nesnesine dönüştürür.
- `stringify(value)`: EMA değerini JSON string'ine dönüştürür.

## 🎨 Örnek Kod

```ema
@server {
    print "Backend calisiyor...";
    std::http::serve();
}

@client {
    <div style: "padding: 20px; color: blue;">
        <h1> "Selam Ema!" </h1>
        js {
            async function selamlama() {
                const res = await fetch("https://api.example.com");
                console.log("Veri geldi!");
            };
            selamlama();
        };
    </div>;
}
```

## 📜 Lisans
Bu proje Evren Mimarları tarafından geliştirilmiştir.
