(let ((let '`(let ((let ',let))
               ,let)))
  `(let ((let ',let))
     ,let))
