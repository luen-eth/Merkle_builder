const { MerkleTree } = require('merkletreejs');
const keccak256 = require('keccak256');
const ethers = require('ethers');
const fs = require('fs');

function writeInfo(id, status, rootHash, fileName, progress, index) {
    const fileInfo = {
        id,
        status,
        rootHash,
        fileName,
        progress,
        index
    };

    const filePath = `./merkle-info/${id}.json`;

    fs.writeFileSync(filePath, JSON.stringify(fileInfo, null, 2), 'utf-8');
    console.log(`Info File Updated: ${filePath}`);
}

function merkleRoot(data, processid) {
    try {
        let addresses = data;

        addresses = addresses.map(item => {
            return {
                address: item.address,
                token: Math.floor(item.token).toString()
            };
        });

        const leafNodes = addresses.map(arr => ethers.utils.solidityKeccak256(["address", "uint256"], [arr.address, arr.token]));

        const merkleTree = new MerkleTree(leafNodes, keccak256, { sortPairs: true });

        const proofs = addresses.map((arr, index) => {
            const proof = merkleTree.getHexProof(leafNodes[index]);
            const progress = ((index + 1) / addresses.length) * 100;
            console.log(` ${processid}  :: Progress : ${progress.toFixed(2)}% `);

            writeInfo(
                processid,
                'loading',
                merkleTree.getHexRoot(),
                `${processid}.json`,
                progress.toFixed(2),
                index
            );

            return { address: arr.address, proof: proof, token: arr.token };
        });

        writeInfo(
            processid,
            'success',
            merkleTree.getHexRoot(),
            `${processid}.json`,
            100,
            0
        );

        console.log(" │ Merkle Tree\n │\n", merkleTree.toString(), "\n");
        console.log("Root Hash ->", merkleTree.getHexRoot(), "\n");

        const outputFileName =  `./outputs/${processid}.json`;

        fs.writeFileSync(outputFileName, JSON.stringify(proofs, null, 2), 'utf-8');
        console.log(`Output File: ${outputFileName} `);

    } catch (err) {
        console.log(err);
    }
}

module.exports = merkleRoot;
