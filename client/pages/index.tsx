// pages/index.tsx

import { useState ,useEffect} from 'react';
import Swal from 'sweetalert2';
import axios from 'axios'
import { useRouter } from 'next/router';



const MerkleTreePage: any = ({ initialData }:any) => {
  
  const router = useRouter();
  const [file, setFile] = useState<File | null>(null);
  const [outputData, setOutputData]:any = useState<any | null>(initialData);
  const [progress, setProgress] = useState<number>(0);

  

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile:any = event.target.files?.[0];
    setFile(selectedFile);
  };


  const generateMerkleTree = async () => {
    try {
      if (!file) {
        console.error('Please upload a file.');
        return;
      }
  
      const fileContent = await readFileContent(file);
      const jsonData = JSON.parse(fileContent);
  
      const formData = new FormData();
      formData.append('jsonFile', new Blob([JSON.stringify(jsonData)], { type: 'application/json' }));
  
      const response = await axios.post('http://localhost:3000/merkleRoot', formData, {
        headers: {
          'Content-Type': 'multipart/form-data'
        }
      });
  
      console.log('Server Response:', response.data);
      
      let {id} = response.data
  
      if(id){
      await  Swal.fire("Request has been received...");
        router.push('/process?processId='+id)
      }else{
        Swal.fire({
          title: "The Internet?",
          text: "Error",
          icon: "error"
        });
      }

    } catch (err) {
      console.error(err);
    }
  };
  

  const readFileContent = (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();

      reader.onload = (event) => {
        if (event.target?.result) {
          resolve(event.target.result as string);
        } else {
          reject(new Error('Error reading file content.'));
        }
      };

      reader.readAsText(file);
    });
  };


  const handleDownloadClick = () => {
    if (outputData) {
      const blob = new Blob([JSON.stringify(outputData, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'output.json';
      a.click();
    }
  };

  const sampleData = `[
    {
      "address": "0x76a7e821e083eceee4e80f9375f00e8ecaf5413c",
      "token": 3921.253389432
    },
    {
      "address": "0x0d1e8d915c1a8d43634dde574e970ed2e2421a1e",
      "token": 7908049.916572514
    },
    {
      "address": "0x76075d952914dde86d4a3784064828477298d705",
      "token": 14878402.545084206
    },
    {
      "address": "0x5B38Da6a701c568545dCfcB03FcB875f56beddC4",
      "token": 32
    }
  ]`;

  return (
      <div className="min-h-screen flex items-center justify-center">
      <div className="p-6 bg-white rounded-lg shadow-md">
        <h1 className="text-3xl font-semibold text-gray-800 mb-6">Merkle Tree Generator</h1>

        <div className="mb-6">
          <label htmlFor="fileInput" className="text-lg mr-4">
            Upload JSON File:
          </label>
          <input type="file" accept=".json" id="fileInput" onChange={handleFileChange} />
        </div>

        <button
          onClick={generateMerkleTree}
          className="bg-green-500 text-white px-4 py-2 text-lg rounded-md cursor-pointer"
        >
          Generate Merkle Tree
        </button>

        

        <div className="mt-6 border border-gray-300 p-4 rounded-md">
          <h2 className="text-lg text-gray-800 mb-4">Example Data Format</h2>
          <pre className="overflow-x-auto">{sampleData}</pre>
        </div>
       
      </div>
    </div>
  );
};

export default MerkleTreePage;
