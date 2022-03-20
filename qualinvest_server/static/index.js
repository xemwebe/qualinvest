// If you only use `npm` you can simply
// import { Chart } from "wasm-demo" and remove `setup` call from `bootstrap.js`.
class Chart {}

const canvas = document.getElementById("canvas");
const coord = document.getElementById("coord");
const status = document.getElementById("status");

let chart = null;

/** Main entry point */
export function main(title, dates, values) {
	let hash = location.hash.substr(1);
    setupUI();
    setupCanvas(title, dates, values);
}

/** This function is used in `bootstrap.js` to setup imports. */
export function setup(WasmChart) {
    Chart = WasmChart;
}

/** Add event listeners. */
function setupUI() {
    window.addEventListener("resize", setupCanvas);
    window.addEventListener("mousemove", onMouseMove);
}

/** Setup canvas to properly handle high DPI and redraw current plot. */
function setupCanvas(title, dates,  values) {
	const dpr = window.devicePixelRatio || 1.0;
    const aspectRatio = canvas.width / canvas.height;
    const size = canvas.parentNode.offsetWidth * 0.8;
    canvas.style.width = size + "px";
    canvas.style.height = size / aspectRatio + "px";
    canvas.width = size;
    canvas.height = size / aspectRatio;
    updatePlot(title, dates, values);
}

/** Update displayed coordinates. */
function onMouseMove(event) {
    if (chart) {
		var text = "Mouse pointer is out of range";

		if(event.target == canvas) {
			let actualRect = canvas.getBoundingClientRect();
			let logicX = event.offsetX * canvas.width / actualRect.width;
			let logicY = event.offsetY * canvas.height / actualRect.height;
			const point = chart.coord(logicX, logicY);
			if (point) {
				let date = new Date(Number(point.x));
				text = `(${date.getDay()+1}.${date.getMonth()+1}.${date.getFullYear()}, ${point.y.toFixed(4)})`
			}
		}
        coord.innerText = text;
    }
}

/** Redraw currently selected plot. */
function updatePlot(title, dates, values) {
    chart = Chart.performance_graph("canvas", title, dates, values);
}
