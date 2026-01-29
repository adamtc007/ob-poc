(list
  "("
  (_)* @function.inside
  ")") @function.around

(map
  "{"
  (_)* @class.inside
  "}") @class.around

(array
  "["
  (_)* @class.inside
  "]") @class.around

(comment)+ @comment.around
