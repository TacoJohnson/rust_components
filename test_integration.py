#!/usr/bin/env python3
"""
Test script for the dsql_decoder integration.
"""

import sys
import os

# Add the parent directory to the path to import frame_loader
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

def test_dsql_decoder():
    """Test the dsql_decoder directly."""
    print("🧪 Testing dsql_decoder directly...")
    
    try:
        import dsql_decoder
        print(f"✅ dsql_decoder imported successfully (version: {dsql_decoder.__version__})")
        
        # Test basic functionality
        print("✅ Frame class available:", hasattr(dsql_decoder, 'Frame'))
        print("✅ from_file method available:", hasattr(dsql_decoder.Frame, 'from_file'))
        
        return True
        
    except ImportError as e:
        print(f"❌ Failed to import dsql_decoder: {e}")
        return False

def test_frame_loader():
    """Test the compatibility wrapper."""
    print("\n🧪 Testing frame_loader compatibility wrapper...")
    
    try:
        import frame_loader
        
        info = frame_loader.get_backend_info()
        print(f"✅ Backend: {info['backend']}")
        print(f"✅ Version: {info['version']}")
        print(f"✅ Performance optimized: {info['performance_optimized']}")
        
        # Test Frame class
        print("✅ Frame class available:", hasattr(frame_loader, 'Frame'))
        print("✅ from_file method available:", hasattr(frame_loader.Frame, 'from_file'))
        
        return True
        
    except Exception as e:
        print(f"❌ Failed to test frame_loader: {e}")
        return False

def main():
    print("🚀 DSQL Decoder Integration Test")
    print("=" * 50)
    
    success = True
    
    # Test dsql_decoder directly
    if not test_dsql_decoder():
        success = False
    
    # Test compatibility wrapper
    if not test_frame_loader():
        success = False
    
    print("\n" + "=" * 50)
    if success:
        print("🎉 All tests passed!")
        print("\n📋 Next Steps:")
        print("1. Test with actual .dsql files")
        print("2. Update the 3D frames tab to use frame_loader")
        print("3. Compare performance with pyfg")
    else:
        print("❌ Some tests failed!")
    
    return success

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
