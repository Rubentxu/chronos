import debugpy
import uvicorn
from fastapi import FastAPI

app = FastAPI()

@app.get("/")
def read_root():
    return {"message": "Hello from Python"}

def main():
    debugpy.listen(("0.0.0.0", 5678))
    print("debugpy listening on :5678")
    uvicorn.run(app, host="0.0.0.0", port=8080)

if __name__ == "__main__":
    main()
