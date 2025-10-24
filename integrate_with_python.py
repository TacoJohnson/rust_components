#!/usr/bin/env python3
"""
Integration script to replace pyfg with dsql_decoder in the Universal Instrument Control system.

This script:
1. Builds and installs the dsql_decoder Rust extension
2. Tests the integration with existing Python code
3. Provides fallback mechanism for compatibility
4. Updates import statements if requested
"""

import os
import sys
import subprocess
import importlib.util
from pathlib import Path
import argparse

def run_command(cmd, cwd=None, check=True):
    """Run a command and handle errors."""
    print(f"🔧 Running: {' '.join(cmd)}")
    if cwd:
        print(f"   in directory: {cwd}")
    
    try:
        result = subprocess.run(cmd, cwd=cwd, check=check, capture_output=True, text=True)
        if result.stdout:
            print(result.stdout)
        return result
    except subprocess.CalledProcessError as e:
        print(f"❌ Command failed with exit code {e.returncode}")
        if e.stdout:
            print("STDOUT:", e.stdout)
        if e.stderr:
            print("STDERR:", e.stderr)
        if check:
            sys.exit(1)
        return e

def check_rust_available():
    """Check if Rust toolchain is available."""
    try:
        subprocess.run(['rustc', '--version'], capture_output=True, check=True)
        subprocess.run(['cargo', '--version'], capture_output=True, check=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def check_maturin_available():
    """Check if maturin is available."""
    try:
        subprocess.run(['maturin', '--version'], capture_output=True, check=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def install_dependencies():
    """Install required dependencies."""
    print("📦 Installing required dependencies...")
    
    if not check_rust_available():
        print("❌ Rust not found. Please install from https://rustup.rs/")
        return False
    
    if not check_maturin_available():
        print("🐍 Installing maturin...")
        run_command([sys.executable, '-m', 'pip', 'install', 'maturin[patchelf]'])
    
    # Install numpy if not available
    try:
        import numpy
        print("✅ numpy is available")
    except ImportError:
        print("🐍 Installing numpy...")
        run_command([sys.executable, '-m', 'pip', 'install', 'numpy'])
    
    return True

def build_dsql_decoder():
    """Build and install the dsql_decoder extension."""
    print("🦀 Building dsql_decoder...")
    
    decoder_dir = Path(__file__).parent / "dsql_decoder"
    
    if not decoder_dir.exists():
        print(f"❌ dsql_decoder directory not found: {decoder_dir}")
        return False
    
    # Build and install in development mode
    run_command(['maturin', 'develop', '--release'], cwd=decoder_dir)
    
    print("✅ dsql_decoder built and installed successfully")
    return True

def test_dsql_decoder():
    """Test the dsql_decoder installation."""
    print("🧪 Testing dsql_decoder installation...")
    
    try:
        import dsql_decoder
        print(f"✅ dsql_decoder imported successfully (version: {dsql_decoder.__version__})")
        
        # Test basic functionality
        print("🔍 Testing basic functionality...")
        
        # Create a minimal test
        test_script = """
import dsql_decoder
print("✅ dsql_decoder.Frame class available:", hasattr(dsql_decoder, 'Frame'))
print("✅ dsql_decoder.Frame.from_file method available:", hasattr(dsql_decoder.Frame, 'from_file'))
"""
        
        exec(test_script)
        return True
        
    except ImportError as e:
        print(f"❌ Failed to import dsql_decoder: {e}")
        return False
    except Exception as e:
        print(f"❌ Error testing dsql_decoder: {e}")
        return False

def create_compatibility_wrapper():
    """Create a compatibility wrapper for seamless pyfg replacement."""
    print("🔧 Creating compatibility wrapper...")
    
    wrapper_content = '''"""
Compatibility wrapper for dsql_decoder to replace pyfg.

This module provides a drop-in replacement for pyfg.frame with the same API
but using the high-performance Rust-based dsql_decoder underneath.

Usage:
    # Instead of: import pyfg.frame
    # Use: import frame_loader as pyfg_frame
    
    frame = frame_loader.Frame.from_file("frame.dsql")
    data = frame.data(decimation=4, field_whitelist=['x', 'y', 'z'])
"""

try:
    # Try to use the fast Rust-based decoder
    import dsql_decoder as _decoder
    RUST_DECODER_AVAILABLE = True
    print("🚀 Using high-performance Rust-based dsql_decoder")
except ImportError:
    # Fall back to pyfg if dsql_decoder is not available
    try:
        import pyfg.frame as _decoder
        RUST_DECODER_AVAILABLE = False
        print("⚠️ Falling back to pyfg (dsql_decoder not available)")
    except ImportError:
        raise ImportError(
            "Neither dsql_decoder nor pyfg is available. "
            "Please install one of them:\\n"
            "  - For Rust decoder: run 'maturin develop' in dsql_decoder/\\n"
            "  - For pyfg: run 'pip install pyfg'"
        )

# Export the Frame class with the same interface
Frame = _decoder.Frame

# Export version information
if RUST_DECODER_AVAILABLE:
    __version__ = getattr(_decoder, '__version__', 'unknown')
    __backend__ = 'rust'
else:
    __version__ = getattr(_decoder, '__version__', 'unknown')
    __backend__ = 'pyfg'

def get_backend_info():
    """Get information about the current backend."""
    return {
        'backend': __backend__,
        'version': __version__,
        'rust_available': RUST_DECODER_AVAILABLE,
        'performance_optimized': RUST_DECODER_AVAILABLE,
    }
'''
    
    # Write the wrapper to the parent directory for easy import
    wrapper_path = Path(__file__).parent.parent / "frame_loader.py"
    
    with open(wrapper_path, 'w') as f:
        f.write(wrapper_content)
    
    print(f"✅ Compatibility wrapper created: {wrapper_path}")
    return wrapper_path

def test_integration():
    """Test integration with the existing codebase."""
    print("🔗 Testing integration with existing codebase...")
    
    # Test the compatibility wrapper
    try:
        sys.path.insert(0, str(Path(__file__).parent.parent))
        import frame_loader
        
        info = frame_loader.get_backend_info()
        print(f"✅ Backend: {info['backend']}")
        print(f"✅ Version: {info['version']}")
        print(f"✅ Performance optimized: {info['performance_optimized']}")
        
        return True
        
    except Exception as e:
        print(f"❌ Integration test failed: {e}")
        return False

def update_imports_in_file(file_path, dry_run=True):
    """Update import statements in a Python file to use the new decoder."""
    print(f"🔍 {'Checking' if dry_run else 'Updating'} imports in: {file_path}")
    
    try:
        with open(file_path, 'r') as f:
            content = f.read()
        
        original_content = content
        
        # Replace pyfg imports
        replacements = [
            ('import pyfg.frame', 'import frame_loader as pyfg_frame'),
            ('from pyfg.frame import', 'from frame_loader import'),
            ('pyfg.frame.Frame', 'frame_loader.Frame'),
        ]
        
        changes_made = []
        for old, new in replacements:
            if old in content:
                if not dry_run:
                    content = content.replace(old, new)
                changes_made.append(f"  {old} → {new}")
        
        if changes_made:
            print(f"{'Would make' if dry_run else 'Made'} changes:")
            for change in changes_made:
                print(change)
            
            if not dry_run:
                with open(file_path, 'w') as f:
                    f.write(content)
                print(f"✅ Updated {file_path}")
        else:
            print("  No changes needed")
        
        return len(changes_made) > 0
        
    except Exception as e:
        print(f"❌ Error processing {file_path}: {e}")
        return False

def find_python_files_with_pyfg():
    """Find Python files that import pyfg."""
    print("🔍 Searching for Python files that use pyfg...")
    
    root_dir = Path(__file__).parent.parent
    python_files = []
    
    for py_file in root_dir.rglob("*.py"):
        try:
            with open(py_file, 'r') as f:
                content = f.read()
                if 'pyfg' in content:
                    python_files.append(py_file)
        except Exception:
            continue  # Skip files we can't read
    
    return python_files

def main():
    parser = argparse.ArgumentParser(description="Integrate dsql_decoder with Universal Instrument Control")
    parser.add_argument('--build', action='store_true', help='Build and install dsql_decoder')
    parser.add_argument('--test', action='store_true', help='Test the installation')
    parser.add_argument('--create-wrapper', action='store_true', help='Create compatibility wrapper')
    parser.add_argument('--update-imports', action='store_true', help='Update import statements in Python files')
    parser.add_argument('--dry-run', action='store_true', help='Show what would be changed without making changes')
    parser.add_argument('--all', action='store_true', help='Do everything (build, test, create wrapper)')
    
    args = parser.parse_args()
    
    if not any([args.build, args.test, args.create_wrapper, args.update_imports, args.all]):
        args.all = True  # Default to doing everything
    
    print("🚀 Universal Instrument Control - Rust Integration")
    print("=" * 60)
    
    success = True
    
    if args.all or args.build:
        if not install_dependencies():
            success = False
        elif not build_dsql_decoder():
            success = False
    
    if success and (args.all or args.test):
        if not test_dsql_decoder():
            success = False
    
    if success and (args.all or args.create_wrapper):
        create_compatibility_wrapper()
        if not test_integration():
            success = False
    
    if success and args.update_imports:
        python_files = find_python_files_with_pyfg()
        if python_files:
            print(f"Found {len(python_files)} Python files with pyfg imports:")
            for py_file in python_files:
                update_imports_in_file(py_file, dry_run=args.dry_run)
        else:
            print("No Python files with pyfg imports found")
    
    if success:
        print("\n🎉 Integration completed successfully!")
        print("\n📋 Next Steps:")
        print("1. Test the 3D frames tab to ensure it works with the new decoder")
        print("2. Compare performance with the old pyfg implementation")
        print("3. Report any issues or performance improvements")
        
        if args.update_imports and args.dry_run:
            print("4. Run with --update-imports (without --dry-run) to apply changes")
    else:
        print("\n❌ Integration failed!")
        sys.exit(1)

if __name__ == "__main__":
    main()
