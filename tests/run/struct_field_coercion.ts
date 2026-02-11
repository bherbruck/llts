// Expected output: item: One, value: 1
type Data = {
  name: string;
  value: i64;
}

const arr: Array<Data> = [
  { name: "One", value: 1 },
]

function main(): void {
  for (const item of arr) {
    print(`item: ${item.name}, value: ${item.value}`);
  }
}
