import os

def chunk_file_for_aico(input_file, chunk_size=10000):
    """
    Splits a file into ASCII chunks of specified length.
    Optimized for streaming into the CDLSS Storm.
    """
    if not os.path.exists(input_file):
        print(f"Error: File '{input_file}' not found.")
        return

    base_name = os.path.splitext(input_file)[0]
    
    try:
        with open(input_file, 'r', encoding='ascii', errors='ignore') as f:
            chunk_count = 0
            while True:
                # Read exactly chunk_size characters
                data = f.read(chunk_size)
                
                if not data:
                    break
                
                chunk_count += 1
                output_filename = f"{base_name}_chunk_{chunk_count:04d}.txt"
                
                with open(output_filename, 'w', encoding='ascii') as chunk_file:
                    chunk_file.write(data)
                
                print(f"Generated {output_filename} ({len(data)} chars)")
                
        print(f"\nProcessing complete. Total chunks: {chunk_count}")

    except Exception as e:
        print(f"An error occurred during chunking: {e}")

if __name__ == "__main__":
    # Replace 'x' with your actual filename (e.g., 'repo_dump.txt')
    file_to_split = "repo_dump.txt" 
    chunk_file_for_aico(file_to_split)