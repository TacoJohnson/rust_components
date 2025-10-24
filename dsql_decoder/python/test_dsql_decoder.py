#!/usr/bin/env python3
"""
Test script for the dsql_decoder Python extension.

This script tests the drop-in replacement functionality for pyfg.
"""

import sys
import os
import numpy as np
import time
from pathlib import Path

def test_dsql_decoder():
    """Test the dsql_decoder module as a pyfg replacement."""
    
    print("🧪 Testing DSQL Decoder - PyFG Replacement")
    print("=" * 50)
    
    try:
        # Try to import our Rust-based decoder
        import dsql_decoder
        print("✅ Successfully imported dsql_decoder")
        print(f"📦 Version: {dsql_decoder.__version__}")
        
    except ImportError as e:
        print(f"❌ Failed to import dsql_decoder: {e}")
        print("💡 Make sure to build the extension first:")
        print("   cd universal_instrument_control/rust_components/dsql_decoder")
        print("   maturin develop")
        return False
    
    # Test with a sample .dsql file (if available)
    test_files = [
        "00000001.dsql",
        "00000000.dsql", 
        "../../../frames/00000001.dsql",
        "../../frames/00000001.dsql",
        "./frames/00000001.dsql",
    ]
    
    test_file = None
    for file_path in test_files:
        if os.path.exists(file_path):
            test_file = file_path
            break
    
    if not test_file:
        print("⚠️ No test .dsql files found. Creating a synthetic test...")
        return test_synthetic_data(dsql_decoder)
    
    print(f"📁 Using test file: {test_file}")
    
    # Test basic frame loading
    try:
        print("\n🔍 Testing frame loading...")
        start_time = time.time()
        frame = dsql_decoder.Frame.from_file(test_file)
        load_time = time.time() - start_time
        
        print(f"✅ Frame loaded successfully in {load_time:.3f}s")
        print(f"📊 Frame number: {frame.number}")
        print(f"📊 Frame type: {frame.type}")
        print(f"📊 Number of pixels: {frame.num_pixels}")
        
    except Exception as e:
        print(f"❌ Failed to load frame: {e}")
        return False
    
    # Test data extraction with different parameters
    test_cases = [
        {"decimation": 1, "field_whitelist": None, "name": "All fields, no decimation"},
        {"decimation": 4, "field_whitelist": None, "name": "All fields, 4x decimation"},
        {"decimation": 1, "field_whitelist": ['x', 'y', 'z'], "name": "XYZ only, no decimation"},
        {"decimation": 2, "field_whitelist": ['x', 'y', 'z', 'intensity'], "name": "XYZ+intensity, 2x decimation"},
        {"decimation": 1, "field_whitelist": ['intensity', 'gain', 'over_range'], "name": "Metadata only"},
    ]
    
    for i, test_case in enumerate(test_cases, 1):
        print(f"\n🧪 Test {i}: {test_case['name']}")
        
        try:
            start_time = time.time()
            data = frame.data(
                decimation=test_case['decimation'],
                field_whitelist=test_case['field_whitelist'],
                time_unit='ticks'  # For pyfg compatibility
            )
            extract_time = time.time() - start_time
            
            print(f"✅ Data extracted in {extract_time:.3f}s")
            print(f"📊 Data type: {type(data)}")
            print(f"📊 Data shape: {data.shape if hasattr(data, 'shape') else 'N/A'}")
            print(f"📊 Data dtype: {data.dtype if hasattr(data, 'dtype') else 'N/A'}")
            
            if hasattr(data, 'dtype') and hasattr(data.dtype, 'names') and data.dtype.names:
                print(f"📊 Available fields: {list(data.dtype.names)}")
                
                # Show sample data for each field
                for field_name in data.dtype.names:
                    field_data = data[field_name]
                    if len(field_data) > 0:
                        print(f"   {field_name}: min={np.min(field_data):.3f}, max={np.max(field_data):.3f}, mean={np.mean(field_data):.3f}")
            
        except Exception as e:
            print(f"❌ Failed to extract data: {e}")
            return False
    
    print("\n🎉 All tests passed!")
    return True

def test_synthetic_data(dsql_decoder):
    """Test with synthetic data when no real .dsql files are available."""
    
    print("🔧 Creating synthetic test data...")
    
    # Create a minimal synthetic .dsql file for testing
    synthetic_file = "test_synthetic.dsql"
    
    try:
        # Create synthetic HWORD data
        # This is a simplified example - real data would be more complex
        hwords = []
        
        # Add a FirstHeader HWORD (control bits = 010 = 2)
        header_hword = bytearray(12)
        header_hword[0] = (2 << 5)  # Control bits in top 3 bits
        hwords.append(bytes(header_hword))
        
        # Add some pixel HWORDs (control bits = 100 = 4 for FirstPixel, 101 = 5 for SubsequentPixel)
        for i in range(100):  # 100 synthetic pixels
            pixel_hword = bytearray(12)
            control_bits = 4 if i == 0 else 5  # FirstPixel for first, SubsequentPixel for rest
            pixel_hword[0] = (control_bits << 5)
            
            # Add some synthetic coordinate data (simplified)
            # In real data, this would be properly encoded fixed-point coordinates
            pixel_hword[1] = i & 0xFF  # Simple test pattern
            pixel_hword[2] = (i >> 8) & 0xFF
            
            hwords.append(bytes(pixel_hword))
        
        # Write synthetic file
        with open(synthetic_file, 'wb') as f:
            for hword in hwords:
                f.write(hword)
        
        print(f"✅ Created synthetic file: {synthetic_file} ({len(hwords)} HWORDs)")
        
        # Test loading the synthetic file
        frame = dsql_decoder.Frame.from_file(synthetic_file)
        print(f"✅ Loaded synthetic frame: {frame.number} pixels, type: {frame.type}")
        
        # Test data extraction
        data = frame.data(decimation=1, field_whitelist=['x', 'y', 'z'])
        print(f"✅ Extracted coordinate data: {data.shape if hasattr(data, 'shape') else 'N/A'}")
        
        # Clean up
        os.remove(synthetic_file)
        print("🧹 Cleaned up synthetic file")
        
        return True
        
    except Exception as e:
        print(f"❌ Synthetic test failed: {e}")
        if os.path.exists(synthetic_file):
            os.remove(synthetic_file)
        return False

def benchmark_comparison():
    """Benchmark comparison with pyfg if available."""
    
    print("\n⚡ Performance Benchmark")
    print("=" * 30)
    
    try:
        import pyfg.frame
        print("✅ pyfg available for comparison")
        
        # TODO: Add benchmark comparison when we have test data
        print("💡 Benchmark comparison requires test .dsql files")
        
    except ImportError:
        print("⚠️ pyfg not available for comparison")

if __name__ == "__main__":
    print("🚀 DSQL Decoder Test Suite")
    print("=" * 60)
    
    success = test_dsql_decoder()
    
    if success:
        benchmark_comparison()
        print("\n✅ Test suite completed successfully!")
        sys.exit(0)
    else:
        print("\n❌ Test suite failed!")
        sys.exit(1)
