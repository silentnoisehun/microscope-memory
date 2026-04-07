import socket
import struct
import json

# Microscope Memory: Binary Spine API Example (Port 6060)

def query_memory(query_text, k=10):
    """
    Connect to the Microscope Memory Binary Spine and perform a recall.
    """
    host = 'localhost'
    port = 6060
    
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.connect((host, port))
            
            # Binary Spine Protocol (Zero JSON Path):
            # 1. Send query length (u32 le)
            # 2. Send query text (utf-8)
            # 3. Receive results...
            
            query_bytes = query_text.encode('utf-8')
            s.sendall(struct.pack('<I', len(query_bytes)))
            s.sendall(query_bytes)
            
            # Placeholder for results parsing (binary headers)
            print(f"Query sent: {query_text}")
            print("Note: The server must be running with 'microscope-mem spine'.")
            
    except ConnectionRefusedError:
        print("Error: Could not connect to Microscope Memory. Is 'microscope-mem spine' running?")

if __name__ == "__main__":
    query_memory("What are the depth levels of the cognitive index?")
