# EMA - ÜNİVERSAL FULL-STACK EKOSİSTEMİ: MASTER PLAN

## 1. DİLİN MİMARİSİ VE SYNTAX (THE EMA LANGUAGE)

**Sözdizimi Felsefesi:** Modern, temiz ve gereksiz sembollerden arındırılmış bir sözdizimi. Değişken tanımlamaları, Rust ve TypeScript'in güçlü yönlerini birleştirir.
**Çift Taraflı Yürütme:** `@server` ve `@client` dekoratörleri sayesinde kodun nerede çalışacağı derleyici düzeyinde garanti edilir.
- **Memory Safety:** Rust'ın sahiplik (ownership) modeli gizli bir şekilde işletilecek. Çöp toplayıcı (Garbage Collector) olmadan bellek sızıntıları önlenecek. 
- **Type Safety:** Derleme anında güçlü tip kontrolü yapılacak, veri yapılarında hata oranı sıfıra indirilecek.

```ema
// example.ema
model Kullanici {
    isim: str
}

@server {
    print "Sunucuda veritabanına bağlanıldı!";
}

@client {
    print "Tarayıcıda arayüz güncellendi!";
}

var main_user = "Admin";
print main_user;
```

## 2. DERLEYİCİ TASARIMI (COMPILER FRONTEND & BACKEND)

- **Lexer/Parser:** Rust ile yazılımı yapılmış lexer, kaynak kodu (Stream of Chars) karakter bazında okuyup jeton (Token) dizisine çevirecek. Parser ise bu token dizisini Rekürsif İniş (Recursive Descent) mantığıyla okuyup Soyut Sözdizimi Ağacına (AST) monte edecek. AST, kodun semantiğini (hangi kod client'ta hangi kod server'da çalışacak) ayrıştıracaktır.
- **AST'den LLVM IR'a:** Semantic Analysis (Değişken kapsamı vb.) ve Type Checking (Tip kontrolü) adımlarından başarıyla geçen AST doğrudan en iyilenmiş bir biçimde LLVM IR'a (Intermediate Representation) dönüştürülecektir.
- **Multi-Target Compilation:** Derleyici, AST içinde `@server` etiketli Node'ları x86_64 veya ARM64 makine kodunu hedefleyen klasik sunucu modüllerinde derlerken, `@client` etiketli kod bloklarını `wasm32-unknown-unknown` hedefine yönelik derleyecek ve WebAssembly çıktısı verecektir. Sonuç olarak aynı kaynaktan iki farklı platform hedefine binary üretilir.

## 3. DİLE GÖMÜLÜ VERİTABANI (EMA-DB)

- **Native Persistence:** Harici bir veritabanı sürücüsüne (PostgreSQL driver vb.) veya ağır bir ORM (Object-Relational Mapping) yapısına ihtiyaç yoktur. Dilde `model` anahtar kelimesi ile yazılan her yapı (struct) derlenme anında kalıcı veri şemasına eşlenir, otomatik migrasyon (auto-migration) devreye girer.
- **Core Engine:** Çekirdekte arka planda sunucu işlemlerinde gömülü bir **DuckDB** veya **SQLite** motoru çalışacaktır. Veritabanı dosyasına doğrudan File System üzerinden erişilirken harici bir ağ bağlantısına gerek kalmaz. Modern analitik için DuckDB önerilir.
- **Syntax:** `Kullanici.insert({isim: "Veli"})` şeklinde tip-güvenli (Type-safe) nesne tabanlı sorgular arka planda AST ayrıştırıcılar tarafından enjekte edilmiş hazırlıklı SQL IR'a dönüştürülüp hafızada işlenir. 

## 4. İŞLETİM SİSTEMİ SEVİYESİNDE ETKİLEŞİM (OS INTERFACE)

- **Syscalls ve I/O:** EmaLang, libc/syscall katmanına doğrudan bağlanan minimal bir standart kütüphane kitiyle gelir. `std::fs` (dosya işlemleri) ve `std::net` modülleri async non-blocking yapıda kurgulanarak işletim sisteminin epoll/kqueue donanım seviyesi çekirdeğiyle doğrudan bağlantı sağlar. Bu da performansı C++ ve Rust düzeyinde tutar.
- **Runtime (Çalışma Zamanı):** Çöp Toplayıcı (Garbage Collector) kullanılmayacaktır. GC pause sorunlarıyla sistem darboğazları yerine derleyici analiz aşamasında "Arena Bellek Ayırıcıları" (Arena Allocators) ve Rust benzeri gizli bir borrow checker yapısı devrede olacaktır. Derleme sırasında heap allocationlar compile-time çözümlendiğinden çalışma zamanında programlar donmaz (stall olmaz). 

## 5. İLK KOD TASLAĞI (THE BOOTSTRAP)
Proje dizini aşağıdaki gibi bir profesyonel yapıyla inşa edilebilir:
```text
C:\Users\PC\Desktop\Ema
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── ast.rs
│   ├── lexer.rs
│   └── parser.rs
├── example.ema
└── EMA_MASTER_PLAN.md
```
Dile ait ana mimarileri sergileyen basit parçalayıcılı kodu Rust dilinde projeye uyarladık. Lexer sembolleri algılarken (`var`, `print`, lexer decorator blokları), Parser bunlardan düz bir AST oluşturabiliyor.
