const { execSync } = require('child_process');
const path = require('path');

/**
 * Compiles an EMA file.
 * @param {string} filePath Path to the .ema file.
 * @returns {string} The output of the compiler.
 */
function compile(filePath) {
    const binName = process.platform === 'win32' ? 'ema_compiler.exe' : 'ema_compiler';
    const binPath = path.join(__dirname, '..', 'target', 'release', binName);
    
    try {
        return execSync(`${binPath} ${filePath}`).toString();
    } catch (error) {
        return error.stderr ? error.stderr.toString() : error.message;
    }
}

module.exports = {
    compile
};
