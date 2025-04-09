
const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');
const merkleRoot = require('./merkleRoot'); 

merkleRoot(workerData.jsonData, workerData.processid, workerData.dataIndex);
parentPort.postMessage('Started : ' + workerData.processid);



