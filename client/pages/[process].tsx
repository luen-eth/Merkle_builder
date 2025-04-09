import axios from 'axios';
import Link from 'next/link';
import { useRouter } from 'next/router';
import { useEffect, useState } from 'react';

const ProcessPage: React.FC = () => {

  const router = useRouter();
  const { processId } = router.query;
  const fileName = `${processId}.json`
  const [processData,setData] = useState({
    error: false , loading:true, err:'', status:'loading',rootHash: null , fileName:'',progress:0
  })
  

  const fullUrl = `${process.env.NEXT_PUBLIC_BASE_URL || ''}${router.asPath}`;

  console.log(processData);
  
  const get= async ()=>{
    console.log(`http://localhost:3000/info/${fileName}`);
    
    const result = await axios.get(`http://localhost:3000/info/${fileName}`)

    console.log(result.data);
    

   if(result.data.data){
    let merkle = result.data.data
    let tmp = {
        error: false , loading:false,err:result.data.error, status:'success',rootHash: merkle.rootHash , fileName: merkle.fileName,progress: merkle.progress
      }
    setData(tmp)
   }else{
    setData({
        error: false ,  loading:false, err:result.data.error,  status:'loading',rootHash: null , fileName:'',progress:0
      })
   }   
 }

 const downloadJsonFile = async () => {
    try {
      const response = await fetch(`http://localhost:3000/download/${fileName}`);
      const data = await response.json();
  
      const jsonData = JSON.stringify(data, null, 2);
      const blob = new Blob([jsonData], { type: 'application/json' });
  
      const link = document.createElement('a');
      link.href = URL.createObjectURL(blob);
      link.download = 'proofs.json';
      link.click();
    } catch (error) {
      console.error('Error downloading JSON file:', error);
    }
  };
  
  useEffect(()=>{
    if(processId){
        setInterval(()=>{
            get()
            },5000)
    }
  },[processId])

  return (
    <div>
          <div className="min-h-screen flex items-center justify-center">
      <div className="p-6 bg-white rounded-lg shadow-md">
        <h1 className="text-3xl font-semibold text-gray-800 mb-6">Merkle Tree Generator</h1>

      {processData.loading? 'Loading...' :  (!processData.error ?
       <div>
        <p className="text-sm text-gray-600 mt-4">Process: {processData.progress}%</p>
          <div className="mt-6">
          <p className="text-lg text-gray-800 mb-4">Root Hash: {processData.rootHash}</p>
          <p className="text-lg text-gray-800 mb-4">Filename: {processData.fileName}</p>
          <p className="text-lg text-gray-800 mb-4">Process Url: {'http://localhost:3001'+fullUrl}</p>

          {
            processData.progress == 100 && <button onClick={downloadJsonFile}
            className="bg-blue-500 text-white px-4 py-2 text-lg rounded-md cursor-pointer"
          >
            Download Proofs
          </button>
          }  
          
          <br></br>  

          <Link className='text-center' href='/'>New Process</Link>

        </div>
        </div>:
            <div>
            
             <Link className='text-center' href='/'>Process not found! </Link>
             <hr></hr>
             <Link className='text-center' href='/'>New Process</Link>
             </div>
      )
}
       
      </div>
    </div>
    </div>
  );
};

export default ProcessPage;