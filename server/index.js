const { MerkleTree } = require('merkletreejs');
const keccak256 = require('keccak256');
const ethers = require('ethers');
const fs = require('fs');
const express = require('express');
const bodyParser = require('body-parser');
const multer = require('multer');
const { v4: uuidv4 } = require('uuid');
const cors = require('cors')
const path = require('path');
const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');

const app = express();
const port = 3000;
app.use(bodyParser.json());
app.use(cors());



const proccess = []



const storage = multer.memoryStorage();
const upload = multer({ storage: storage });

app.get('/info/:fileName', (req, res) => {
    const fileName = req.params.fileName;
    const filePath = path.join(__dirname, 'merkle-info', fileName);
  
    if (fs.existsSync(filePath)) {
      const fileContent = fs.readFileSync(filePath, 'utf-8');
      try {
        const fileInfo = JSON.parse(fileContent);
        res.json({ status: true, data: fileInfo });
      } catch (err) {
        console.error('JSON Parsing Error:', err);
        res.status(200).json({ status: false, error: 'JSON Parsing Error' });
      }
    } else {
      res.status(200).json({ status: false, error: 'File not found' });
    }
  });
  
app.use('/download', express.static(path.join(__dirname, 'outputs')));

app.post('/merkleRoot', upload.single('jsonFile'), async (req, res) => {
    const dataIndex = proccess.length;
    const processid = uuidv4();
    const jsonData = JSON.parse(req.file.buffer.toString('utf8'));
  
    let infoData = {
        id: processid,
        status: 'loading',
        rootHash: '0x',
        fileName: `${processid}.json`,
        progress: 0,
        index: dataIndex,
    }

    const infoFilePath = `./merkle-info/${infoData.fileName}`;

   await fs.writeFileSync(infoFilePath, JSON.stringify(infoData, null, 2), 'utf-8');

  
    res.json({ status: true, id: processid });
  

      const worker = new Worker('./worker.js', {
        workerData: { jsonData, processid, dataIndex },
      });
  
      worker.on('message', (result) => {
        console.log('Worker Result:', result);
      });
   
  });



  
  app.listen(port, () => {
    console.log(`Server listening at http://localhost:${port}`);
  });