#!/usr/bin/env node

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const binName = process.platform === 'win32' ? 'ema_compiler.exe' : 'ema_compiler';
const binPath = path.join(__dirname, binName);

if (!fs.existsSync(binPath)) {
    console.error(`\x1b[31m[ERROR] Ema V1.0 Engine not found at ${binPath}\x1b[0m`);
    console.error(`\x1b[33mHave you run npm install?\x1b[0m`);
    process.exit(1);
}

const args = process.argv.slice(2);

const result = spawnSync(binPath, args, { stdio: 'inherit' });

if (result.error) {
    console.error('\x1b[31m[EMA FATAL] Failed to start the Emalang compiler.\x1b[0m');
    console.error(result.error);
    process.exit(1);
}

process.exit(result.status || 0);
