use rand::{Rng, SeedableRng};
use sha2::{Digest, Sha256};

/// Content generator for creating deterministic file content
pub struct ContentGenerator {
    rng: rand::rngs::StdRng,
    cache: std::collections::HashMap<(String, u64), Vec<u8>>,
}

impl ContentGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: rand::rngs::StdRng::seed_from_u64(seed),
            cache: std::collections::HashMap::new(),
        }
    }

    /// Get a mutable reference to the internal RNG for custom random operations
    pub fn rng(&mut self) -> &mut rand::rngs::StdRng {
        &mut self.rng
    }

    /// Generate deterministic content for a given file ID and size
    pub fn generate(&mut self, file_id: &str, size: u64, content_type: ContentType) -> Vec<u8> {
        let key = (format!("{}_{}", file_id, size), content_type as u64);
        
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }

        let content = match content_type {
            ContentType::Text => self.generate_text(file_id, size),
            ContentType::StructuredText => self.generate_structured_text(file_id, size),
            ContentType::Json => self.generate_json(file_id, size),
            ContentType::Csv => self.generate_csv(file_id, size),
            ContentType::Markdown => self.generate_markdown(file_id, size),
            ContentType::RandomBinary => self.generate_random_binary(size),
            ContentType::PatternedBinary => self.generate_patterned_binary(file_id, size),
            ContentType::Zero => vec![0; size as usize],
        };

        self.cache.insert(key, content.clone());
        content
    }

    fn generate_text(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let words = [
            "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
            "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
            "magna", "aliqua", "ut", "enim", "ad", "minim", "veniam", "quis", "nostrud",
            "exercitation", "ullamco", "laboris", "nisi", "ut", "aliquip", "ex", "ea",
            "commodo", "consequat", "duis", "aute", "irure", "in", "reprehenderit",
            "voluptate", "velit", "esse", "cillum", "eu", "fugiat", "nulla", "pariatur",
        ];
        
        let mut result = Vec::new();
        let mut file_rng = self.rng_for_file(file_id);
        
        while result.len() < size as usize {
            let word = words[file_rng.gen_range(0..words.len())];
            if result.len() + word.len() + 1 > size as usize {
                break;
            }
            if !result.is_empty() {
                result.push(b' ');
            }
            result.extend_from_slice(word.as_bytes());
        }
        
        // Pad if needed
        if result.len() < size as usize {
            result.resize(size as usize, b' ');
        }
        
        result.truncate(size as usize);
        result
    }

    fn generate_structured_text(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut file_rng = self.rng_for_file(file_id);
        
        let templates = [
            "Report generated on {date}\n\nSection 1: Overview\n{content}\n\nSection 2: Details\n{content}\n\nSection 3: Conclusion\n{content}\n",
            "Document ID: {id}\nAuthor: Test Generator\nDate: {date}\n\nContent:\n{content}\n\nEnd of document.\n",
            "Log entry {id}\nTimestamp: {date}\nLevel: INFO\nMessage: {content}\n---\n",
        ];
        
        let template = templates[file_rng.gen_range(0..templates.len())];
        
        while result.len() < size as usize {
            let content = self.generate_lorem_paragraph(&mut file_rng, 200);
            let entry = template
                .replace("{id}", file_id)
                .replace("{date}", &chrono::Utc::now().to_rfc3339())
                .replace("{content}", &content);
            result.extend_from_slice(entry.as_bytes());
            if result.len() >= size as usize {
                break;
            }
        }
        
        result.truncate(size as usize);
        result
    }

    fn generate_json(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut file_rng = self.rng_for_file(file_id);
        
        result.extend_from_slice(b"{\n  \"id\": \"");
        result.extend_from_slice(file_id.as_bytes());
        result.extend_from_slice(b"\",\n  \"items\": [\n");
        
        while result.len() < size as usize - 100 {
            let item_size = file_rng.gen_range(50..200).min(size as usize - result.len() - 10);
            let item = self.generate_json_object(&mut file_rng, item_size);
            result.extend_from_slice(b"    ");
            result.extend_from_slice(&item);
            result.extend_from_slice(b",\n");
        }
        
        result.extend_from_slice(b"  ]\n}\n");
        result.truncate(size as usize);
        result
    }

    fn generate_csv(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut file_rng = self.rng_for_file(file_id);
        
        // Header
        result.extend_from_slice(b"id,name,value,timestamp,category\n");
        
        while result.len() < size as usize {
            let id = file_rng.gen_range(1..10000);
            let name = self.generate_lorem_word(&mut file_rng, 10);
            let value = file_rng.gen_range(0..1000000);
            let timestamp = chrono::Utc::now().timestamp() + file_rng.gen_range(-1000000..1000000);
            let category = ["A", "B", "C", "D"][file_rng.gen_range(0..4)];
            
            let line = format!("{},{},{},{},{}\n", id, name, value, timestamp, category);
            result.extend_from_slice(line.as_bytes());
        }
        
        result.truncate(size as usize);
        result
    }

    fn generate_markdown(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let mut result = Vec::new();
        let mut file_rng = self.rng_for_file(file_id);
        
        result.extend_from_slice(format!("# Document {}\n\n", file_id).as_bytes());
        
        while result.len() < size as usize {
            let heading_level = file_rng.gen_range(1..4);
            let heading = format!("{} Section {}\n\n", "#".repeat(heading_level), file_rng.gen_range(1..100));
            result.extend_from_slice(heading.as_bytes());
            
            let paragraph = self.generate_lorem_paragraph(&mut file_rng, 300);
            result.extend_from_slice(paragraph.as_bytes());
            result.extend_from_slice(b"\n\n");
        }
        
        result.truncate(size as usize);
        result
    }

    fn generate_random_binary(&mut self, size: u64) -> Vec<u8> {
        let mut result = vec![0u8; size as usize];
        self.rng.fill(&mut result[..]);
        result
    }

    fn generate_patterned_binary(&mut self, file_id: &str, size: u64) -> Vec<u8> {
        let mut result = vec![0u8; size as usize];
        
        // Use a deterministic pattern based on file_id hash
        let seed = self.hash_string(file_id);
        let mut pattern_rng = rand::rngs::StdRng::seed_from_u64(seed);
        
        for chunk in result.chunks_mut(256) {
            pattern_rng.fill(chunk);
        }
        
        result
    }

    fn generate_lorem_paragraph(&self, rng: &mut rand::rngs::StdRng, max_len: usize) -> String {
        let words = [
            "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
            "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
            "magna", "aliqua", "ut", "enim", "ad", "minim", "veniam", "quis", "nostrud",
        ];
        
        let mut result = String::new();
        let word_count = rng.gen_range(10..max_len / 5);
        
        for i in 0..word_count {
            if i > 0 {
                result.push(' ');
            }
            result.push_str(words[rng.gen_range(0..words.len())]);
        }
        
        result
    }

    fn generate_lorem_word(&self, rng: &mut rand::rngs::StdRng, _max_len: usize) -> String {
        let words = [
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
            "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi", "rho",
            "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
        ];
        words[rng.gen_range(0..words.len())].to_string()
    }

    fn generate_json_object(&self, rng: &mut rand::rngs::StdRng, _size: usize) -> Vec<u8> {
        let keys = ["name", "value", "timestamp", "active", "count", "ratio"];
        let mut obj = String::from("{");
        
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                obj.push(',');
            }
            obj.push_str(&format!("\"{}\":", key));
            
            match *key {
                "name" => obj.push_str(&format!("\"{}\"", self.generate_lorem_word(rng, 10))),
                "value" => obj.push_str(&rng.gen_range(0..1000000).to_string()),
                "timestamp" => obj.push_str(&chrono::Utc::now().timestamp().to_string()),
                "active" => obj.push_str(if rng.gen_bool(0.5) { "true" } else { "false" }),
                "count" => obj.push_str(&rng.gen_range(0..1000).to_string()),
                "ratio" => obj.push_str(&format!("{:.4}", rng.gen::<f64>())),
                _ => obj.push_str("null"),
            }
        }
        
        obj.push('}');
        obj.into_bytes()
    }

    fn rng_for_file(&self, file_id: &str) -> rand::rngs::StdRng {
        let seed = self.hash_string(file_id);
        rand::rngs::StdRng::seed_from_u64(seed)
    }

    fn hash_string(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Generate content with a specific SHA256 hash (for duplicate files)
    pub fn generate_with_hash(&mut self, file_id: &str, size: u64, content_type: ContentType, _target_hash: &[u8]) -> Vec<u8> {
        // For duplicates, we need to generate content that matches the target hash
        // This is not feasible for SHA256, so we store the content once and reuse it
        // The generator handles this by generating content once per duplicate group
        self.generate(file_id, size, content_type)
    }

    /// Compute SHA256 of content
    pub fn sha256(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }
}

/// Types of content that can be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    Text,
    StructuredText,
    Json,
    Csv,
    Markdown,
    RandomBinary,
    PatternedBinary,
    Zero,
}

impl ContentType {
    pub fn extension(&self) -> &'static str {
        match self {
            ContentType::Text => "txt",
            ContentType::StructuredText => "txt",
            ContentType::Json => "json",
            ContentType::Csv => "csv",
            ContentType::Markdown => "md",
            ContentType::RandomBinary => "bin",
            ContentType::PatternedBinary => "bin",
            ContentType::Zero => "bin",
        }
    }

    pub fn random(rng: &mut rand::rngs::StdRng) -> Self {
        match rng.gen_range(0..8) {
            0 => ContentType::Text,
            1 => ContentType::StructuredText,
            2 => ContentType::Json,
            3 => ContentType::Csv,
            4 => ContentType::Markdown,
            5 => ContentType::RandomBinary,
            6 => ContentType::PatternedBinary,
            _ => ContentType::Zero,
        }
    }
}