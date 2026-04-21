const express = require('express');
const app = express();
const port = 8080;

app.get('/', (req, res) => {
  res.json({ message: 'Hello from Node.js' });
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Node.js server listening on :${port}`);
});
