#!/usr/bin/env python3
"""
Build script for Universal Instrument Control Rust components.

This script automates the building of all Rust components including:
- shared library
- dsql_decoder Python extension
- framegrabber application
"""

import subprocess
import sys
import os
from pathlib import Path
import argparse

def run_command(cmd, cwd=None, check=True):
    """Run a command and handle errors."""
    print(f"🔧 Running: {' '.join(cmd)}")
    if cwd:
        print(f"   in directory: {cwd}")

    try:
        result = subprocess.run(cmd, cwd=cwd, check=check, capture_output=True, text=True, encoding='utf-8', errors='replace')
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

def check_rust_installation():
    """Check if Rust is properly installed."""
    print("🦀 Checking Rust installation...")
    
    try:
        result = subprocess.run(['rustc', '--version'], capture_output=True, text=True, check=True)
        print(f"✅ {result.stdout.strip()}")
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("❌ Rust not found. Please install Rust from https://rustup.rs/")
        sys.exit(1)
    
    try:
        result = subprocess.run(['cargo', '--version'], capture_output=True, text=True, check=True)
        print(f"✅ {result.stdout.strip()}")
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("❌ Cargo not found. Please install Rust from https://rustup.rs/")
        sys.exit(1)

def check_python_dependencies():
    """Check if required Python dependencies are installed."""
    print("🐍 Checking Python dependencies...")
    
    required_packages = ['maturin', 'numpy']
    missing_packages = []
    
    for package in required_packages:
        try:
            __import__(package)
            print(f"✅ {package} is installed")
        except ImportError:
            missing_packages.append(package)
            print(f"❌ {package} is missing")
    
    if missing_packages:
        print(f"\n💡 Install missing packages with:")
        print(f"   pip install {' '.join(missing_packages)}")
        
        if input("Install missing packages now? (y/N): ").lower().startswith('y'):
            run_command([sys.executable, '-m', 'pip', 'install'] + missing_packages)
        else:
            sys.exit(1)

def build_shared_library():
    """Build the shared library."""
    print("\n📚 Building shared library...")
    
    shared_dir = Path(__file__).parent / "shared"
    run_command(['cargo', 'build', '--release'], cwd=shared_dir)
    
    print("✅ Shared library built successfully")

def build_dsql_decoder():
    """Build the dsql_decoder Python extension."""
    print("\n🐍 Building dsql_decoder Python extension...")

    decoder_dir = Path(__file__).parent / "dsql_decoder"

    # Build with maturin using the correct Python interpreter
    run_command(['py', '-3.11', '-m', 'maturin', 'build', '--release', '--interpreter', 'py'], cwd=decoder_dir)

    print("✅ dsql_decoder built successfully")

def install_dsql_decoder():
    """Install the dsql_decoder Python extension."""
    print("\n📦 Installing dsql_decoder...")

    # Find the wheel file
    wheels_dir = Path(__file__).parent / "target" / "wheels"
    wheel_files = list(wheels_dir.glob("dsql_decoder-*.whl"))

    if not wheel_files:
        print("❌ No wheel file found. Build first with --component decoder")
        return False

    wheel_file = wheel_files[0]  # Use the most recent one
    print(f"📦 Found wheel: {wheel_file}")

    # Install the wheel using the correct Python interpreter
    run_command(['py', '-3.11', '-m', 'pip', 'install', str(wheel_file), '--force-reinstall'])

    print("✅ dsql_decoder installed successfully")
    return True

def build_framegrabber():
    """Build the framegrabber application."""
    print("\n🚀 Building framegrabber application...")
    
    framegrabber_dir = Path(__file__).parent / "framegrabber"
    run_command(['cargo', 'build', '--release'], cwd=framegrabber_dir)
    
    print("✅ Framegrabber built successfully")

def test_dsql_decoder():
    """Test the dsql_decoder Python extension."""
    print("\n🧪 Testing dsql_decoder...")
    
    test_script = Path(__file__).parent / "dsql_decoder" / "python" / "test_dsql_decoder.py"
    
    if test_script.exists():
        run_command([sys.executable, str(test_script)], check=False)
    else:
        print("⚠️ Test script not found, skipping tests")

def main():
    parser = argparse.ArgumentParser(description="Build Universal Instrument Control Rust components")
    parser.add_argument('--component', choices=['shared', 'decoder', 'framegrabber', 'all'], 
                       default='all', help='Component to build')
    parser.add_argument('--install', action='store_true', 
                       help='Install dsql_decoder in development mode')
    parser.add_argument('--test', action='store_true', 
                       help='Run tests after building')
    parser.add_argument('--skip-checks', action='store_true', 
                       help='Skip dependency checks')
    
    args = parser.parse_args()
    
    print("🏗️ Universal Instrument Control - Rust Components Builder")
    print("=" * 60)
    
    # Check dependencies
    if not args.skip_checks:
        check_rust_installation()
        check_python_dependencies()
    
    # Build components
    if args.component in ['shared', 'all']:
        build_shared_library()
    
    if args.component in ['decoder', 'all']:
        build_dsql_decoder()
        
        if args.install:
            install_dsql_decoder()
    
    if args.component in ['framegrabber', 'all']:
        build_framegrabber()
    
    # Run tests
    if args.test and args.component in ['decoder', 'all']:
        test_dsql_decoder()
    
    print("\n🎉 Build completed successfully!")
    
    # Show usage instructions
    print("\n📋 Usage Instructions:")
    print("=" * 30)
    
    if args.component in ['decoder', 'all']:
        if args.install:
            print("✅ dsql_decoder is installed and ready to use:")
            print("   import dsql_decoder")
            print("   frame = dsql_decoder.Frame.from_file('frame.dsql')")
        else:
            print("💡 To install dsql_decoder in development mode:")
            print("   python build.py --component decoder --install")
    
    if args.component in ['framegrabber', 'all']:
        framegrabber_exe = Path(__file__).parent / "framegrabber" / "target" / "release" / "framegrabber"
        if os.name == 'nt':  # Windows
            framegrabber_exe = framegrabber_exe.with_suffix('.exe')
        
        print(f"✅ Framegrabber executable: {framegrabber_exe}")
        print("   Run with: ./framegrabber --help")

if __name__ == "__main__":
    main()
