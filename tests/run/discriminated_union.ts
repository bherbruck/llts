// Expected output: 75\n24

interface Circle {
  kind: "circle";
  radius: f64;
}

interface Rectangle {
  kind: "rectangle";
  width: f64;
  height: f64;
}

type Shape = Circle | Rectangle;

function area(s: Shape): f64 {
  switch (s.kind) {
    case "circle":
      return s.radius * s.radius * 3;
    case "rectangle":
      return s.width * s.height;
    default:
      return 0;
  }
}

function main(): void {
  const c: Shape = { kind: "circle", radius: 5 };
  print(area(c));
  const r: Shape = { kind: "rectangle", width: 4, height: 6 };
  print(area(r));
}
